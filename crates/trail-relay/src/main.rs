//! trail-relay: play Westward Trail together on a home network.
//!
//! It does two jobs on one port:
//!   * serves the web game (index.html, main.js, trail.wasm) so every device
//!     loads it from here, and
//!   * relays multiplayer lines over a WebSocket at /ws: every line one player
//!     sends goes to everyone else. The relay knows nothing about the game.
//!
//!   cargo run --release -p trail-relay -- --web web --port 8850
//!
//! Then open http://<this computer>:8850 on each device. Cardputers find the
//! relay by themselves: they broadcast "westward?" to UDP port 8851 and the
//! relay answers "westward-relay <port>".
//!
//! It's meant for a home network: there are no accounts and no encryption,
//! so don't put it on the internet.

use std::{
    collections::HashMap,
    fs,
    io::{ErrorKind, Read, Write},
    net::{TcpListener, TcpStream, UdpSocket},
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        mpsc::{Receiver, Sender, channel},
    },
    thread,
    time::Duration,
};

use tungstenite::{Message, WebSocket};

/// Where Cardputers ask "is there a relay here?".
const DISCOVERY_PORT: u16 = 8851;

/// Most players at once, across all games on this relay.
const MAX_CLIENTS: usize = 32;
/// Game lines are short, except the shared screen's (up to about 1500);
/// anything longer is dropped.
const MAX_LINE: usize = 2048;

type Clients = Arc<Mutex<HashMap<u64, Sender<String>>>>;

struct Options {
    web: PathBuf,
    port: u16,
    bind: String,
}

fn options() -> Options {
    let mut o = Options {
        web: PathBuf::from("web"),
        port: 8850,
        bind: String::from("0.0.0.0"),
    };
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let value = args.get(i + 1).cloned().unwrap_or_default();
        match args[i].as_str() {
            "--web" => o.web = PathBuf::from(value),
            "--port" => o.port = value.parse().unwrap_or(o.port),
            "--bind" => o.bind = value,
            "-h" | "--help" => {
                println!("trail-relay [--web DIR] [--port 8850] [--bind 0.0.0.0]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown option {other} (try --help)");
                std::process::exit(2);
            }
        }
        i += 2;
    }
    o
}

fn main() {
    let o = options();
    if !o.web.join("trail.wasm").exists() {
        eprintln!(
            "{} has no trail.wasm. Build it first with ./web/build.sh",
            o.web.display()
        );
        std::process::exit(1);
    }
    let addr = format!("{}:{}", o.bind, o.port);
    let listener = TcpListener::bind(&addr).unwrap_or_else(|e| {
        eprintln!("can't listen on {addr}: {e}");
        std::process::exit(1);
    });
    println!(
        "Westward Trail relay on http://{addr}/ serving {}",
        o.web.display()
    );
    let port = o.port;
    thread::spawn(move || answer_discovery(port));
    let clients: Clients = Arc::default();
    let web = Arc::new(o.web);
    let mut next_id = 0u64;
    for stream in listener.incoming().flatten() {
        next_id += 1;
        let (id, clients, web) = (next_id, clients.clone(), web.clone());
        thread::spawn(move || {
            if let Err(e) = handle(stream, id, &clients, &web) {
                eprintln!("connection {id}: {e}");
            }
        });
    }
}

/// Answer Cardputers looking for a relay on the network.
/// It listens on every address: broadcasts only reach a socket on 0.0.0.0,
/// even when the web side is tied to one address.
fn answer_discovery(port: u16) {
    let socket = match UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "no discovery on UDP {DISCOVERY_PORT}: {e} (Cardputers will need the address)"
            );
            return;
        }
    };
    let mut buf = [0u8; 64];
    while let Ok((n, from)) = socket.recv_from(&mut buf) {
        if &buf[..n] == b"westward?" {
            let _ = socket.send_to(format!("westward-relay {port}").as_bytes(), from);
        }
    }
}

/// The path from the first line of an HTTP request, without reading it off the socket.
fn peek_path(stream: &TcpStream) -> std::io::Result<String> {
    let mut buf = [0u8; 1024];
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    for _ in 0..50 {
        let n = stream.peek(&mut buf)?;
        if n == 0 {
            break;
        }
        let head = String::from_utf8_lossy(&buf[..n]);
        if let Some(first) = head.lines().next().filter(|_| head.contains("\r\n")) {
            let path = first.split_whitespace().nth(1).unwrap_or("/");
            return Ok(String::from(path.split('?').next().unwrap_or("/")));
        }
        thread::sleep(Duration::from_millis(20));
    }
    Ok(String::from("/"))
}

fn handle(stream: TcpStream, id: u64, clients: &Clients, web: &Path) -> Result<(), String> {
    let path = peek_path(&stream).map_err(|e| e.to_string())?;
    if path == "/ws" {
        relay(stream, id, clients)
    } else {
        serve(stream, &path, web).map_err(|e| e.to_string())
    }
}

// ---------------------------------------------------------------- the web game

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript",
        "wasm" => "application/wasm",
        "png" => "image/png",
        "css" => "text/css",
        "json" => "application/json",
        _ => "application/octet-stream",
    }
}

fn serve(mut stream: TcpStream, path: &str, web: &Path) -> std::io::Result<()> {
    // Read (and ignore) the rest of the request.
    let mut buf = [0u8; 4096];
    let _ = stream.read(&mut buf)?;
    let (status, ctype, body) = if path == "/relay.json" {
        // The page checks for this to know it can play together.
        ("200 OK", "application/json", b"{\"relay\":true}".to_vec())
    } else {
        let rel = path.trim_start_matches('/');
        let rel = if rel.is_empty() { "index.html" } else { rel };
        let file = Path::new(rel);
        let safe = file.components().all(|c| matches!(c, Component::Normal(_)));
        match safe.then(|| fs::read(web.join(file))) {
            Some(Ok(body)) => ("200 OK", content_type(file), body),
            _ => ("404 Not Found", "text/plain", b"not found".to_vec()),
        }
    };
    let head = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nCache-Control: no-cache\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&body)?;
    stream.flush()
}

// ---------------------------------------------------------------- the relay

fn relay(stream: TcpStream, id: u64, clients: &Clients) -> Result<(), String> {
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| e.to_string())?;
    let mut ws = tungstenite::accept(stream).map_err(|e| e.to_string())?;
    // Short reads, so lines from other players go out promptly.
    ws.get_ref()
        .set_read_timeout(Some(Duration::from_millis(30)))
        .map_err(|e| e.to_string())?;
    let (tx, rx) = channel();
    {
        let mut all = clients.lock().map_err(|_| "lock")?;
        if all.len() >= MAX_CLIENTS {
            let _ = ws.close(None);
            return Err(String::from("too many players"));
        }
        all.insert(id, tx);
        println!("player {id} connected ({} online)", all.len());
    }
    let result = pump(&mut ws, id, clients, &rx);
    if let Ok(mut all) = clients.lock() {
        all.remove(&id);
        println!("player {id} left ({} online)", all.len());
    }
    result
}

fn pump(
    ws: &mut WebSocket<TcpStream>,
    id: u64,
    clients: &Clients,
    rx: &Receiver<String>,
) -> Result<(), String> {
    loop {
        match ws.read() {
            Ok(Message::Text(text)) => {
                let text = text.as_str();
                if text.len() <= MAX_LINE {
                    let all = clients.lock().map_err(|_| "lock")?;
                    for (other, tx) in all.iter() {
                        if *other != id {
                            let _ = tx.send(String::from(text));
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => return Ok(()),
            Ok(_) => {}
            Err(tungstenite::Error::Io(e))
                if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
            Err(tungstenite::Error::ConnectionClosed | tungstenite::Error::AlreadyClosed) => {
                return Ok(());
            }
            Err(e) => return Err(e.to_string()),
        }
        while let Ok(line) = rx.try_recv() {
            ws.send(Message::text(line)).map_err(|e| e.to_string())?;
        }
    }
}
