//! Playing together over Wi-Fi.
//!
//! Host: the Cardputer starts a hotspot (`WestwardTrail-XXXX`, new random
//! password each time), serves the web game from flash so phones and
//! computers can join in a browser, and relays game lines over a WebSocket.
//!
//! Join: the Cardputer joins a Wi-Fi network. On another Cardputer's hotspot
//! the game is at 192.168.4.1; on home Wi-Fi it asks the network for a
//! trail-relay (UDP broadcast "westward?" to port 8851). Then it speaks just
//! enough WebSocket to send and receive game lines.
//!
//! The game itself never sees any of this: it gets lines in and hands lines
//! out, like it does with beeps.

use std::{
    io::{ErrorKind, Read, Write as _},
    net::{Ipv4Addr, SocketAddrV4, TcpStream, UdpSocket},
    sync::{
        mpsc::{Receiver, Sender},
        Arc, Mutex,
    },
    time::Duration,
};

use anyhow::{anyhow, bail, Result};
use esp_idf_hal::modem::Modem;
use esp_idf_svc::{
    eventloop::EspSystemEventLoop,
    http::{
        server::{Configuration as HttpConfig, EspHttpServer},
        Method,
    },
    io::Write,
    nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault},
    wifi::{
        AccessPointConfiguration, AuthMethod, BlockingWifi, ClientConfiguration, Configuration,
        EspWifi,
    },
    ws::FrameType,
};
use qrcodegen::{QrCode, QrCodeEcc};
use trail_core::HostInfo;

// The web game, served to phones and computers that join our hotspot.
// Build it first with ./web/build.sh.
const INDEX_HTML: &[u8] = include_bytes!("../../web/index.html");
const MAIN_JS: &[u8] = include_bytes!("../../web/main.js");
const TRAIL_WASM: &[u8] = include_bytes!("../../web/trail.wasm");

const HOTSPOT_PREFIX: &str = "WestwardTrail-";
const DISCOVERY_PORT: u16 = 8851;
const MAX_LINE: usize = 512;

/// From the game to the network.
pub enum Command {
    Host,
    Join {
        ssid: String,
        password: String,
        relay: String,
    },
    Send(String),
}

/// From the network to the game.
pub enum Event {
    Online(bool),
    HostInfo(HostInfo),
    Status(String),
    Line(String),
    /// The Wi-Fi network joined last time (and the relay address typed for it), to offer again.
    SavedNetwork(String, String),
}

fn random_u32() -> u32 {
    unsafe { esp_idf_svc::sys::esp_random() }
}

/// A password that's easy to read off a tiny screen: no 0/o, 1/l/i.
fn new_password() -> String {
    const LETTERS: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";
    (0..8)
        .map(|_| LETTERS[random_u32() as usize % LETTERS.len()] as char)
        .collect()
}

fn qr(text: &str) -> (Vec<bool>, usize) {
    match QrCode::encode_text(text, QrCodeEcc::Low) {
        Ok(code) => {
            let size = code.size();
            let modules = (0..size)
                .flat_map(|y| (0..size).map(move |x| (x, y)))
                .map(|(x, y)| code.get_module(x, y))
                .collect();
            (modules, size as usize)
        }
        Err(_) => (Vec::new(), 0),
    }
}

/// Runs forever on its own thread, waiting for the game to ask for hosting or joining.
pub fn run(
    modem: Modem,
    sysloop: EspSystemEventLoop,
    nvs: EspDefaultNvsPartition,
    events: Sender<Event>,
    commands: Receiver<Command>,
) {
    let mut saved = EspNvs::new(nvs.clone(), "wifi", true).ok();
    if let Some(nvs) = saved.as_ref() {
        if let Some(ssid) = read(nvs, "ssid") {
            let relay = read(nvs, "relay").unwrap_or_default();
            let _ = events.send(Event::SavedNetwork(ssid, relay));
        }
    }
    let mut wifi = match EspWifi::new(modem, sysloop.clone(), Some(nvs))
        .and_then(|w| BlockingWifi::wrap(w, sysloop))
    {
        Ok(w) => w,
        Err(e) => {
            log::error!("Wi-Fi unavailable: {e:?}");
            return;
        }
    };
    loop {
        let Ok(command) = commands.recv() else {
            return;
        };
        let result = match command {
            Command::Host => host(&mut wifi, &events, &commands),
            Command::Join {
                ssid,
                password,
                relay,
            } => join(
                &mut wifi,
                saved.as_mut(),
                (&ssid, &password, &relay),
                &events,
                &commands,
            ),
            Command::Send(_) => continue, // not connected: drop it
        };
        if let Err(e) = result {
            log::warn!("play together stopped: {e:#}");
            let _ = events.send(Event::Status(format!("{e}")));
        }
        let _ = events.send(Event::Online(false));
        let _ = wifi.stop();
    }
}

fn read(nvs: &EspNvs<NvsDefault>, key: &str) -> Option<String> {
    let mut buf = [0u8; 70];
    nvs.get_str(key, &mut buf).ok().flatten().map(str::to_string)
}

// ---------------------------------------------------------------- hosting

type Peers = Arc<Mutex<Vec<esp_idf_svc::http::server::ws::EspHttpWsDetachedSender>>>;

fn host(
    wifi: &mut BlockingWifi<EspWifi<'static>>,
    events: &Sender<Event>,
    commands: &Receiver<Command>,
) -> Result<()> {
    let ssid = format!("{HOTSPOT_PREFIX}{:04X}", random_u32() & 0xFFFF);
    let password = new_password();
    let _ = wifi.stop();
    wifi.set_configuration(&Configuration::AccessPoint(AccessPointConfiguration {
        ssid: ssid.as_str().try_into().map_err(|_| anyhow!("bad hotspot name"))?,
        password: password.as_str().try_into().map_err(|_| anyhow!("bad password"))?,
        auth_method: AuthMethod::WPA2Personal,
        channel: 6,
        max_connections: 8,
        ..Default::default()
    }))?;
    wifi.start()?;
    wifi.wait_netif_up()?;
    log::info!("hotspot {ssid} is up");

    let (modules, size) = qr(&format!("WIFI:T:WPA;S:{ssid};P:{password};;"));
    let _ = events.send(Event::HostInfo(HostInfo {
        ssid,
        password,
        address: String::from("http://192.168.4.1"),
        qr: modules,
        qr_size: size,
    }));

    let mut server = EspHttpServer::new(&HttpConfig {
        max_open_sockets: 12,
        lru_purge_enable: true,
        stack_size: 10 * 1024,
        ..Default::default()
    })?;
    for (path, body, kind) in [
        ("/", INDEX_HTML, "text/html; charset=utf-8"),
        ("/index.html", INDEX_HTML, "text/html; charset=utf-8"),
        ("/main.js", MAIN_JS, "text/javascript"),
        ("/trail.wasm", TRAIL_WASM, "application/wasm"),
        ("/relay.json", b"{\"relay\":true}".as_slice(), "application/json"),
    ] {
        server.fn_handler(path, Method::Get, move |req| {
            req.into_response(200, Some("OK"), &[("Content-Type", kind)])?
                .write_all(body)
        })?;
    }

    let peers: Peers = Arc::default();
    let (ws_peers, ws_events) = (peers.clone(), Mutex::new(events.clone()));
    server.ws_handler("/ws", move |ws| {
        let mut all = ws_peers.lock().map_err(|_| anyhow!("lock"))?;
        if ws.is_new() {
            all.push(ws.create_detached_sender()?);
            log::info!("player joined ({} connected)", all.len());
            return Ok::<(), anyhow::Error>(());
        }
        if ws.is_closed() {
            let gone = ws.session();
            all.retain(|p| p.session() != gone);
            log::info!("player left ({} connected)", all.len());
            return Ok(());
        }
        let (_, len) = ws.recv(&mut [])?;
        if len > MAX_LINE {
            return Ok(());
        }
        let mut buf = vec![0u8; len];
        let (kind, _) = ws.recv(&mut buf)?;
        if !matches!(kind, FrameType::Text(_)) {
            return Ok(());
        }
        // The C side ends text with a zero; drop it.
        let text = String::from_utf8_lossy(&buf).trim_end_matches('\0').to_string();
        let from = ws.session();
        all.retain_mut(|p| p.session() == from || p.send(FrameType::Text(false), text.as_bytes()).is_ok());
        if let Ok(tx) = ws_events.lock() {
            let _ = tx.send(Event::Line(text));
        }
        Ok(())
    })?;
    let _ = events.send(Event::Online(true));

    // Lines from the game on this Cardputer go to everyone connected.
    while let Ok(command) = commands.recv() {
        match command {
            Command::Send(line) => {
                if let Ok(mut all) = peers.lock() {
                    all.retain_mut(|p| p.send(FrameType::Text(false), line.as_bytes()).is_ok());
                }
            }
            Command::Host | Command::Join { .. } => {
                // Asked again from the menu: keep hosting (the game shows the same info).
            }
        }
    }
    drop(server);
    Ok(())
}

// ---------------------------------------------------------------- joining

fn join(
    wifi: &mut BlockingWifi<EspWifi<'static>>,
    saved: Option<&mut EspNvs<NvsDefault>>,
    (ssid, password, relay): (&str, &str, &str),
    events: &Sender<Event>,
    commands: &Receiver<Command>,
) -> Result<()> {
    let password = if password.is_empty() {
        saved
            .as_deref()
            .filter(|n| read(n, "ssid").as_deref() == Some(ssid))
            .and_then(|n| read(n, "pass"))
            .unwrap_or_default()
    } else {
        String::from(password)
    };
    let status = |text: String| {
        let _ = events.send(Event::Status(text));
    };
    status(format!("Joining {ssid}"));
    let _ = wifi.stop();
    wifi.set_configuration(&Configuration::Client(ClientConfiguration {
        ssid: ssid.try_into().map_err(|_| anyhow!("that Wi-Fi name is too long"))?,
        password: password
            .as_str()
            .try_into()
            .map_err(|_| anyhow!("that password is too long"))?,
        auth_method: if password.is_empty() {
            AuthMethod::None
        } else {
            AuthMethod::WPA2Personal
        },
        ..Default::default()
    }))?;
    wifi.start()?;
    wifi.connect()
        .map_err(|_| anyhow!("Couldn't join {ssid}. Check the name and password."))?;
    wifi.wait_netif_up()?;
    if let Some(n) = saved {
        let _ = n.set_str("ssid", ssid);
        let _ = n.set_str("pass", &password);
        let _ = n.set_str("relay", relay);
    }

    let relay = if ssid.starts_with(HOTSPOT_PREFIX) {
        SocketAddrV4::new(Ipv4Addr::new(192, 168, 4, 1), 80)
    } else if !relay.is_empty() {
        // Typed by hand: "10.0.0.5" or "10.0.0.5:8850".
        let (ip, port) = relay.split_once(':').unwrap_or((relay, "8850"));
        let ip: Ipv4Addr = ip
            .trim()
            .parse()
            .map_err(|_| anyhow!("\"{relay}\" isn't an address like 10.0.0.5"))?;
        SocketAddrV4::new(ip, port.trim().parse().unwrap_or(8850))
    } else {
        status(String::from("Looking for the relay"));
        find_relay().ok_or_else(|| anyhow!("No Westward Trail relay on {ssid}."))?
    };
    status(format!("Connecting to {}", relay.ip()));
    let mut ws = WsClient::connect(relay)?;
    let _ = events.send(Event::Online(true));
    loop {
        for line in ws.poll()? {
            let _ = events.send(Event::Line(line));
        }
        while let Ok(command) = commands.try_recv() {
            if let Command::Send(line) = command {
                ws.send(&line)?;
            }
        }
    }
}

/// Ask the network for a trail-relay: broadcast "westward?", wait for "westward-relay <port>".
fn find_relay() -> Option<SocketAddrV4> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.set_broadcast(true).ok()?;
    socket.set_read_timeout(Some(Duration::from_millis(700))).ok()?;
    let mut buf = [0u8; 64];
    for _ in 0..6 {
        let _ = socket.send_to(b"westward?", ("255.255.255.255", DISCOVERY_PORT));
        if let Ok((n, std::net::SocketAddr::V4(from))) = socket.recv_from(&mut buf) {
            let reply = String::from_utf8_lossy(&buf[..n]);
            if let Some(port) = reply.strip_prefix("westward-relay ").and_then(|p| p.trim().parse().ok()) {
                return Some(SocketAddrV4::new(*from.ip(), port));
            }
        }
    }
    None
}

/// Just enough of a WebSocket client for short text lines.
struct WsClient {
    stream: TcpStream,
    buf: Vec<u8>,
}

impl WsClient {
    fn connect(addr: SocketAddrV4) -> Result<Self> {
        let mut stream = TcpStream::connect_timeout(&addr.into(), Duration::from_secs(5))?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let request = format!(
            "GET /ws HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: d2VzdHdhcmQtdHJhaWwhIQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        );
        stream.write_all(request.as_bytes())?;
        let mut head = Vec::new();
        let mut byte = [0u8; 1];
        while !head.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte)? == 0 || head.len() > 2048 {
                bail!("the relay hung up");
            }
            head.push(byte[0]);
        }
        if !head.starts_with(b"HTTP/1.1 101") {
            bail!("that isn't a Westward Trail relay");
        }
        stream.set_read_timeout(Some(Duration::from_millis(20)))?;
        Ok(WsClient {
            stream,
            buf: Vec::new(),
        })
    }

    /// Text lines that have arrived (none if nothing has).
    fn poll(&mut self) -> Result<Vec<String>> {
        let mut chunk = [0u8; 512];
        match self.stream.read(&mut chunk) {
            Ok(0) => bail!("Lost the connection"),
            Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
            Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(e) => return Err(e.into()),
        }
        let mut lines = Vec::new();
        // Server frames aren't masked: [fin+opcode] [length] [extended length] [payload].
        while self.buf.len() >= 2 {
            let opcode = self.buf[0] & 0x0F;
            let (len, start) = match self.buf[1] & 0x7F {
                126 if self.buf.len() >= 4 => (u16::from_be_bytes([self.buf[2], self.buf[3]]) as usize, 4),
                127 if self.buf.len() >= 10 => {
                    let mut b = [0u8; 8];
                    b.copy_from_slice(&self.buf[2..10]);
                    (u64::from_be_bytes(b) as usize, 10)
                }
                126 | 127 => break,
                n => (n as usize, 2),
            };
            if self.buf.len() < start + len {
                break;
            }
            let payload: Vec<u8> = self.buf[start..start + len].to_vec();
            self.buf.drain(..start + len);
            match opcode {
                0x1 => lines.push(String::from_utf8_lossy(&payload).into_owned()),
                0x8 => bail!("The game closed"),
                0x9 => self.frame(0xA, &payload)?,
                _ => {}
            }
        }
        Ok(lines)
    }

    fn send(&mut self, line: &str) -> Result<()> {
        self.frame(0x1, line.as_bytes())
    }

    /// Clients must mask what they send.
    fn frame(&mut self, opcode: u8, payload: &[u8]) -> Result<()> {
        let mask = random_u32().to_be_bytes();
        let mut out = vec![0x80 | opcode];
        match payload.len() {
            n if n < 126 => out.push(0x80 | n as u8),
            n => {
                out.push(0x80 | 126);
                out.extend_from_slice(&(n as u16).to_be_bytes());
            }
        }
        out.extend_from_slice(&mask);
        out.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.stream.write_all(&out)?;
        Ok(())
    }
}
