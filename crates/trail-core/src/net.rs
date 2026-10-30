//! Playing together: the lobby, the wagon race and the wagon train.
//! The one-party games (one wagon, democracy, hunting) live in [`crate::party`].
//!
//! The game never touches the network. Like beeps, it hands out short text
//! lines ([`Game::take_net`]) for the platform to send, and takes lines in
//! ([`Game::net_in`]). A relay passes every line to everyone else, so the
//! lines say which game they belong to (the host's id) and who sent them.
//!
//! The lines (fields split by spaces; a name is always last, so it can have spaces):
//!
//! ```text
//! adv    <host> <mode> <players> <cheats> <name>        a host advertising its lobby
//! join   <host> <id> <name>                             asking to join
//! member <host> <id> <name>                             the host listing who's in
//! start  <host> <seed> <mode> <cheats> <gentle> <month> the race begins
//! pos    <host> <id> <miles> <day> <alive>              a wagon's progress
//! news   <host> <id> <text>                             "reached Fort Kearney"
//! done   <host> <id> <points>                           made it to Oregon
//! out    <host> <id>                                    everyone in the wagon is gone
//! msg    <host> <id> <n>                                a message from the list
//! give   <host> <from> <to> <item> <amount>             wagon train: supplies for another wagon
//! leave  <host> <id>                                    a player left
//! bye    <host>                                         the host closed the game
//! ```

use alloc::{format, string::String, vec::Vec};

use crate::{
    Game, Key, Mood, Scene, Trip,
    gfx::{self, Color, Font, Frame, W},
    rng::Rng,
    setup, ui,
};

pub const MAX_PLAYERS: usize = 8;
/// Cardputers name their hotspots like this.
pub const HOTSPOT_PREFIX: &str = "WestwardTrail-";
const ADVERT_MS: u32 = 1500;
const POS_MS: u32 = 3000;
/// A lobby or player not heard from for this long is gone or lost.
const LOST_MS: u32 = 20_000;

pub const MESSAGES: [&str; 8] = [
    "Race you to Oregon!",
    "Need food!",
    "Need oxen!",
    "Wait for me!",
    "Watch out for the river!",
    "Good luck!",
    "Help!",
    "Woo-hoo!",
];

/// Colors for players' wagons on the map, in roster order.
const COLORS: [Color; MAX_PLAYERS] = [
    gfx::RED,
    gfx::BLUE,
    gfx::rgb(240, 200, 0),
    gfx::PURPLE,
    gfx::ORANGE,
    gfx::rgb(0, 170, 170),
    gfx::rgb(240, 120, 180),
    gfx::WHITE,
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Race,
    Train,
    Democracy,
    Shared,
    Hunt,
}

impl Mode {
    const ALL: [Mode; 5] = [
        Mode::Race,
        Mode::Train,
        Mode::Democracy,
        Mode::Shared,
        Mode::Hunt,
    ];

    fn name(self) -> &'static str {
        match self {
            Mode::Race => "Wagon race",
            Mode::Train => "Wagon train",
            Mode::Democracy => "Democracy",
            Mode::Shared => "One wagon, many drivers",
            Mode::Hunt => "Hunt together",
        }
    }

    /// Everyone drives their own wagon.
    pub fn own_wagons(self) -> bool {
        matches!(self, Mode::Race | Mode::Train)
    }

    /// The host runs the only trip and everyone else watches its screen.
    pub fn one_party(self) -> bool {
        !self.own_wagons()
    }

    fn about(self) -> &'static str {
        match self {
            Mode::Race => {
                "Everyone drives their own wagon with the same luck. First to Oregon wins."
            }
            Mode::Train => {
                "Everyone drives their own wagon, and wagons close together can give each other supplies."
            }
            Mode::Democracy => {
                "One wagon. Each player is a party member, and the party votes on every choice."
            }
            Mode::Shared => {
                "One wagon. Players take turns driving: the turn passes each time the wagon sets off."
            }
            Mode::Hunt => {
                "Everyone hunts in the same field at once. Who brings back the most meat?"
            }
        }
    }

    fn code(self) -> &'static str {
        match self {
            Mode::Race => "race",
            Mode::Train => "train",
            Mode::Democracy => "vote",
            Mode::Shared => "shared",
            Mode::Hunt => "hunt",
        }
    }

    fn from_code(s: &str) -> Mode {
        Mode::ALL
            .into_iter()
            .find(|m| m.code() == s)
            .unwrap_or(Mode::Race)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PeerState {
    Waiting,
    Racing,
    Done(u32),
    Out,
}

#[derive(Clone, Debug)]
pub struct Peer {
    pub id: u32,
    pub name: String,
    pub miles: u32,
    pub day: u32,
    pub alive: u32,
    pub state: PeerState,
    /// `Net::clock` when we last heard from them.
    seen: u32,
}

#[derive(Clone, Debug)]
pub struct Lobby {
    pub host: u32,
    pub name: String,
    pub mode: Mode,
    pub players: u32,
    pub cheats: bool,
    seen: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub mode: Mode,
    pub cheats: bool,
    pub gentle: bool,
    /// 1-5: March to July, like the month menu.
    pub month: u32,
    /// Democracy vote timer in seconds; 0 means no timer.
    pub vote_secs: u32,
}

/// Asks for the platform's network (Cardputer Wi-Fi), from [`Game::take_net_requests`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NetRequest {
    /// Start a hotspot here and relay for everyone who joins.
    Host,
    /// Join a Wi-Fi network and find the game on it. An empty password means
    /// "use the one you saved for this network".
    /// `relay` is "address[:port]" of a trail-relay; empty means search for one.
    Join {
        ssid: String,
        password: String,
        relay: String,
    },
}

/// What the platform can do, from [`Game::set_net_caps`].
#[derive(Clone, Default, Debug)]
pub struct Caps {
    pub host: bool,
    pub join: bool,
    /// A network joined before, offered as a quick choice.
    pub saved_ssid: String,
    /// The relay address used on that network, if one was typed.
    pub saved_relay: String,
}

/// The hotspot we're hosting, for the screen that tells others how to join.
#[derive(Clone, Default, Debug)]
pub struct HostInfo {
    pub ssid: String,
    pub password: String,
    pub address: String,
    /// QR code modules (true = dark), `qr_size` on a side. Empty means no QR.
    pub qr: Vec<bool>,
    pub qr_size: usize,
}

pub struct Net {
    /// The platform says the relay is reachable.
    pub online: bool,
    pub caps: Caps,
    pub requests: Vec<NetRequest>,
    pub host_info: Option<HostInfo>,
    /// What the platform is doing right now ("Connecting...").
    pub status: String,
    pub me: u32,
    pub name: String,
    out: Vec<String>,
    /// The game we're in: its host's id (ours, when hosting).
    pub host: Option<u32>,
    pub players: Vec<Peer>,
    pub lobbies: Vec<Lobby>,
    pub settings: Settings,
    pub racing: bool,
    /// The one-party games: whose turn, votes, the shared screen.
    pub party: crate::party::Party,
    clock: u32,
    advert_in: u32,
    pos_in: u32,
}

/// A player id from the game's seed. SplitMix64 mixes every bit, so nearby
/// seeds (like 2 and 3) still give different ids.
fn player_id(seed: u64) -> u32 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) as u32) | 1
}

impl Net {
    pub fn new(seed: u64) -> Self {
        Net {
            online: false,
            caps: Caps::default(),
            requests: Vec::new(),
            host_info: None,
            status: String::new(),
            me: player_id(seed),
            name: String::new(),
            out: Vec::new(),
            host: None,
            players: Vec::new(),
            lobbies: Vec::new(),
            settings: Settings {
                mode: Mode::Race,
                cheats: true,
                gentle: true,
                month: 2,
                vote_secs: 20,
            },
            racing: false,
            party: crate::party::Party::default(),
            clock: 0,
            advert_in: 0,
            pos_in: 0,
        }
    }

    pub fn take(&mut self) -> Vec<String> {
        let mut out = core::mem::take(&mut self.out);
        out.append(self.party.out.get_mut());
        out
    }

    pub(crate) fn send(&mut self, line: String) {
        if self.online && self.out.len() < 64 {
            self.out.push(line);
        }
    }

    pub fn hosting(&self) -> bool {
        self.host == Some(self.me)
    }

    /// True when the host has turned cheats off for this race.
    pub fn cheats_blocked(&self) -> bool {
        self.racing && !self.settings.cheats
    }

    fn me_peer(&self) -> Peer {
        Peer {
            id: self.me,
            name: self.name.clone(),
            miles: 0,
            day: 0,
            alive: 5,
            state: PeerState::Waiting,
            seen: self.clock,
        }
    }

    pub(crate) fn color(&self, id: u32) -> Color {
        let i = self.players.iter().position(|p| p.id == id).unwrap_or(0);
        COLORS[i % MAX_PLAYERS]
    }

    pub(crate) fn name_of(&self, id: u32) -> String {
        self.players
            .iter()
            .find(|p| p.id == id)
            .map_or_else(|| String::from("Someone"), |p| p.name.clone())
    }
}

/// The screens of the "Play together" menu.
pub enum Screen {
    Offline,
    /// Our hotspot: name, password, QR code.
    Hotspot,
    /// Typing the network name, password and (on home Wi-Fi) the relay's address.
    JoinWifi {
        fields: [String; 3],
        field: usize,
    },
    /// Waiting for the platform to connect.
    Connecting,
    Name(String),
    Menu,
    Setup,
    Waiting,
    Standings {
        last: bool,
    },
    Message,
    /// Wagon train: giving supplies to another wagon (once one is picked).
    Give(Option<u32>),
    /// Watching the host's screen in a one-party game.
    Remote,
    /// A watcher's own menu.
    RemoteMenu,
    /// Hunting together: who got what.
    HuntResults(Vec<(u32, u32)>),
}

// ---------------------------------------------------------------- lines in and out

fn line(kind: &str, host: u32, rest: &str) -> String {
    if rest.is_empty() {
        format!("{kind} {host}")
    } else {
        format!("{kind} {host} {rest}")
    }
}

fn say(g: &mut Game, kind: &str, rest: &str) {
    if let Some(host) = g.net.host {
        g.net.send(line(kind, host, rest));
    }
}

/// Words after the first `n`, joined back together (names can have spaces).
fn tail(words: &[&str], n: usize) -> String {
    words.get(n..).map_or_else(String::new, |w| w.join(" "))
}

fn num(words: &[&str], i: usize) -> Option<u32> {
    words.get(i)?.parse().ok()
}

/// One line from the relay.
pub fn receive(g: &mut Game, text: &str) {
    let w: Vec<&str> = text.split_whitespace().collect();
    let (Some(&kind), Some(host)) = (w.first(), num(&w, 1)) else {
        return;
    };
    let clock = g.net.clock;
    if kind == "adv" {
        if host == g.net.me {
            return;
        }
        let lobby = Lobby {
            host,
            mode: Mode::from_code(w.get(2).copied().unwrap_or("")),
            players: num(&w, 3).unwrap_or(1),
            cheats: w.get(4) == Some(&"1"),
            name: tail(&w, 5),
            seen: clock,
        };
        match g.net.lobbies.iter_mut().find(|l| l.host == host) {
            Some(l) => *l = lobby,
            None => g.net.lobbies.push(lobby),
        }
        return;
    }
    if g.net.host != Some(host) {
        return; // someone else's game
    }
    if crate::party::receive(g, kind, &w) {
        return;
    }
    let id = num(&w, 2).unwrap_or(0);
    if let Some(p) = g.net.players.iter_mut().find(|p| p.id == id) {
        p.seen = clock;
    }
    match kind {
        "join" if g.net.hosting() && !g.net.racing => {
            if !g.net.players.iter().any(|p| p.id == id) && g.net.players.len() < MAX_PLAYERS {
                let mut p = g.net.me_peer();
                p.id = id;
                p.name = tail(&w, 3);
                g.notify(format!("{} joined!", p.name));
                g.net.players.push(p);
                g.beep(784, 60);
            }
            send_roster(g);
        }
        "member" if !g.net.hosting() => {
            if !g.net.players.iter().any(|p| p.id == id) {
                let mut p = g.net.me_peer();
                p.id = id;
                p.name = tail(&w, 3);
                g.net.players.push(p);
            }
        }
        "start" if !g.net.hosting() && !g.net.racing => {
            let s = &mut g.net.settings;
            s.mode = Mode::from_code(w.get(3).copied().unwrap_or(""));
            s.cheats = w.get(4) == Some(&"1");
            s.gentle = w.get(5) == Some(&"1");
            s.month = num(&w, 6).unwrap_or(2).clamp(1, 5);
            let seed = num(&w, 2).unwrap_or(1);
            g.pending = Some(begin(g, seed));
        }
        "pos" => {
            if let Some(p) = g.net.players.iter_mut().find(|p| p.id == id) {
                p.miles = num(&w, 3).unwrap_or(p.miles);
                p.day = num(&w, 4).unwrap_or(p.day);
                p.alive = num(&w, 5).unwrap_or(p.alive);
                if p.state == PeerState::Waiting {
                    p.state = PeerState::Racing;
                }
            }
        }
        "news" if id != g.net.me => {
            let who = g.net.name_of(id);
            g.notify(format!("{who} {}", tail(&w, 3)));
        }
        "done" => {
            let points = num(&w, 3).unwrap_or(0);
            if let Some(p) = g.net.players.iter_mut().find(|p| p.id == id) {
                p.state = PeerState::Done(points);
            }
            if id != g.net.me {
                let who = g.net.name_of(id);
                g.notify(format!("{who} made it to Oregon!"));
                g.tune(&[(659, 80), (880, 120)]);
            }
        }
        "out" => {
            if let Some(p) = g.net.players.iter_mut().find(|p| p.id == id) {
                p.state = PeerState::Out;
                p.alive = 0;
            }
        }
        "give" if num(&w, 3) == Some(g.net.me) => {
            let (item, amount) = (num(&w, 4).unwrap_or(99) as usize, num(&w, 5).unwrap_or(0));
            if let Some(&(_, name, _)) = GIFTS.get(item) {
                *gift_stock(g, item) += amount;
                let who = g.net.name_of(id);
                g.notify(format!("{who} gave you {amount} {name}!"));
                g.tune(&[(784, 60), (988, 90)]);
            }
        }
        "msg" if id != g.net.me => {
            let n = num(&w, 3).unwrap_or(0) as usize;
            if let Some(m) = MESSAGES.get(n) {
                let who = g.net.name_of(id);
                g.notify(format!("{who}: {m}"));
                g.beep(988, 40);
            }
        }
        "leave" => {
            let who = g.net.name_of(id);
            g.net.players.retain(|p| p.id != id);
            g.notify(format!("{who} left"));
            if g.net.hosting() {
                send_roster(g);
            }
        }
        "bye" if !g.net.hosting() => {
            if g.net.racing {
                g.notify("The host left; keep racing!");
            } else {
                g.net.host = None;
                g.net.players.clear();
                g.notify("The host closed the game");
                g.pending = Some(Scene::Online(Screen::Menu));
            }
        }
        _ => {}
    }
}

fn send_roster(g: &mut Game) {
    let roster: Vec<(u32, String)> = g
        .net
        .players
        .iter()
        .map(|p| (p.id, p.name.clone()))
        .collect();
    for (id, name) in roster {
        say(g, "member", &format!("{id} {name}"));
    }
}

/// Every frame: adverts, progress reports, forgetting lost lobbies.
pub fn tick(g: &mut Game, ms: u32) {
    let n = &mut g.net;
    n.clock = n.clock.wrapping_add(ms);
    let clock = n.clock;
    n.lobbies
        .retain(|l| clock.wrapping_sub(l.seen) < LOST_MS / 4);
    if n.hosting() && !n.racing {
        n.advert_in = n.advert_in.saturating_sub(ms);
        if n.advert_in == 0 {
            n.advert_in = ADVERT_MS;
            let (me, mode, count, cheats) =
                (n.me, n.settings.mode, n.players.len(), n.settings.cheats);
            let name = n.name.clone();
            n.send(format!(
                "adv {me} {} {count} {} {name}",
                mode.code(),
                u8::from(cheats)
            ));
            // Repeat the roster now and then, for anyone who missed it.
            send_roster(g);
        }
    }
    crate::party::tick(g, ms);
    if g.net.racing && g.net.host.is_some() && g.net.settings.mode.own_wagons() {
        g.net.pos_in = g.net.pos_in.saturating_sub(ms);
        if g.net.pos_in == 0 {
            g.net.pos_in = POS_MS;
            let (me, miles, day, alive) = (g.net.me, g.t.miles, g.t.day, g.t.alive_count());
            if let Some(p) = g.net.players.iter_mut().find(|p| p.id == me) {
                p.miles = miles;
                p.day = day;
                p.alive = alive;
            }
            say(g, "pos", &format!("{me} {miles} {day} {alive}"));
        }
    }
}

// ---------------------------------------------------------------- hooks from the rest of the game

/// Tell the other wagons something ("reached Fort Kearney").
pub fn news(g: &mut Game, text: &str) {
    if g.net.racing && g.net.settings.mode.own_wagons() {
        let me = g.net.me;
        say(g, "news", &format!("{me} {text}"));
    }
}

pub fn finished(g: &mut Game, points: u32) {
    if g.net.racing && g.net.settings.mode.own_wagons() {
        let me = g.net.me;
        if let Some(p) = g.net.players.iter_mut().find(|p| p.id == me) {
            p.state = PeerState::Done(points);
            p.miles = g.t.miles;
        }
        say(g, "done", &format!("{me} {points}"));
    }
}

pub fn wagon_lost(g: &mut Game) {
    if g.net.racing && g.net.settings.mode.own_wagons() {
        let me = g.net.me;
        if let Some(p) = g.net.players.iter_mut().find(|p| p.id == me) {
            p.state = PeerState::Out;
            p.alive = 0;
        }
        say(g, "out", &format!("{me}"));
    }
}

/// Leave whatever game we're in.
pub fn leave(g: &mut Game) {
    if g.net.host.is_some() {
        if g.net.hosting() {
            say(g, "bye", "");
        } else {
            let me = g.net.me;
            say(g, "leave", &format!("{me}"));
        }
    }
    g.net.host = None;
    g.net.players.clear();
    g.net.racing = false;
}

/// Everyone starts the same trail from the same seed.
fn begin(g: &mut Game, seed: u32) -> Scene {
    let s = g.net.settings;
    g.t = Trip::new();
    g.t.mood = if s.gentle {
        Mood::Gentle
    } else {
        Mood::Classic
    };
    g.cheats.used = false;
    g.rng = Rng::new(u64::from(seed));
    g.net.racing = true;
    g.net.pos_in = 500;
    for p in &mut g.net.players {
        p.state = PeerState::Racing;
        p.miles = 0;
    }
    g.msgs.clear();
    g.tune(&[(523, 100), (659, 100), (784, 100), (1047, 200)]);
    if s.mode.one_party() && !g.net.hosting() {
        return crate::party::begin_watching(g);
    }
    let month = setup::MONTH_NAMES[s.month as usize - 1];
    match s.mode {
        Mode::Race => g.say(
            "The race is on!",
            format!(
                "Everyone starts from Independence in {month}, with the same luck. Press R on the trail to see who's ahead. First to Oregon wins, but points count too!"
            ),
        ),
        Mode::Train => g.say(
            "The wagon train sets off!",
            format!(
                "Everyone leaves Independence in {month}. Press R on the trail to see the other wagons. Wagons within {NEAR} miles of each other can give supplies: press G there."
            ),
        ),
        Mode::Shared => g.say(
            "One wagon, many drivers",
            "You all share one wagon. The driver makes every choice, and the turn passes to the next player each time the wagon sets off. You drive first!",
        ),
        Mode::Democracy => g.say(
            "Democracy",
            "You all share one wagon, and every big choice is a vote: everyone presses their pick. Most votes wins, and the leader breaks ties. The leader types names and shops.",
        ),
        Mode::Hunt => {}
    }
    if s.mode.one_party() {
        crate::party::begin_host(g);
    }
    if s.mode == Mode::Hunt {
        return hunt_round(g);
    }
    Scene::Occupation
}

/// Hunting together: a fresh field, somewhere new along the trail.
fn hunt_round(g: &mut Game) -> Scene {
    g.t.miles = g.rng.below(g.data.trail_length());
    g.t.sup.ammo = 999;
    Scene::Hunt(alloc::boxed::Box::new(crate::hunt::HuntState::new(g)))
}

/// Hunting together: the round is over.
pub fn hunt_over(g: &mut Game, results: Vec<(u32, u32)>) -> Scene {
    g.tune(&[(523, 80), (659, 80), (784, 160)]);
    Scene::Online(Screen::HuntResults(results))
}

/// A party member's name when playing together: your own name leads your
/// wagon, and in a one-party game each player is a party member.
pub fn party_name(g: &Game, i: usize) -> String {
    if !g.net.racing {
        return String::new();
    }
    if g.net.settings.mode.one_party() {
        return g
            .net
            .players
            .get(i)
            .map_or_else(String::new, |p| p.name.clone());
    }
    if i == 0 {
        g.net.name.clone()
    } else {
        String::new()
    }
}

// ---------------------------------------------------------------- wagon train gifts

/// Wagons this close can give each other supplies.
pub const NEAR: u32 = 50;

/// What one wagon can give another: (menu text, name in the note, amount).
const GIFTS: [(&str, &str, u32); 6] = [
    ("50 pounds of food", "pounds of food", 50),
    ("1 ox", "ox", 1),
    ("20 bullets", "bullets", 20),
    ("1 set of clothing", "set of clothing", 1),
    ("1 wagon wheel", "wagon wheel", 1),
    ("1 wagon axle", "wagon axle", 1),
];

fn gift_stock(g: &mut Game, item: usize) -> &mut u32 {
    let s = &mut g.t.sup;
    match item {
        0 => &mut s.food,
        1 => &mut s.oxen,
        2 => &mut s.ammo,
        3 => &mut s.clothing,
        4 => &mut s.wheels,
        _ => &mut s.axles,
    }
}

fn give_key(g: &mut Game, to: Option<u32>, k: Key) -> Scene {
    if k == Key::Escape {
        return Scene::Online(Screen::Standings { last: false });
    }
    let Some(n) = ui::digit(k) else {
        return Scene::Online(Screen::Give(to));
    };
    let Some(to) = to else {
        // Picking the wagon.
        let others: Vec<u32> = g
            .net
            .players
            .iter()
            .filter(|p| p.id != g.net.me)
            .map(|p| p.id)
            .collect();
        return match others.get(n.wrapping_sub(1)) {
            Some(&id) => {
                let p = g
                    .net
                    .players
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| (p.miles, p.state));
                let far = p.map_or(u32::MAX, |(miles, _)| miles.abs_diff(g.t.miles));
                if far > NEAR {
                    g.notify(format!("Too far away: {far} miles!"));
                    Scene::Online(Screen::Give(None))
                } else if matches!(p, Some((_, PeerState::Done(_) | PeerState::Out))) {
                    g.notify("That wagon's trip is over");
                    Scene::Online(Screen::Give(None))
                } else {
                    Scene::Online(Screen::Give(Some(id)))
                }
            }
            None => Scene::Online(Screen::Give(None)),
        };
    };
    let Some(&(_, name, amount)) = GIFTS.get(n.wrapping_sub(1)) else {
        return Scene::Online(Screen::Give(Some(to)));
    };
    let item = n - 1;
    if *gift_stock(g, item) < amount {
        g.notify("You don't have enough");
        return Scene::Online(Screen::Give(Some(to)));
    }
    *gift_stock(g, item) -= amount;
    let me = g.net.me;
    say(g, "give", &format!("{me} {to} {item} {amount}"));
    let who = g.net.name_of(to);
    g.notify(format!("You gave {who} {amount} {name}"));
    g.beep(784, 50);
    Scene::Online(Screen::Give(Some(to)))
}

// ---------------------------------------------------------------- screens

pub fn open(g: &mut Game) -> Scene {
    if !g.net.online {
        return Scene::Online(Screen::Offline);
    }
    if g.net.racing {
        return Scene::Online(Screen::Standings { last: false });
    }
    if g.net.name.is_empty() {
        return Scene::Online(Screen::Name(String::new()));
    }
    Scene::Online(Screen::Menu)
}

pub fn key(g: &mut Game, s: Screen, k: Key) -> Scene {
    match s {
        Screen::Offline => {
            let caps = g.net.caps.clone();
            match ui::digit(k) {
                Some(1) if caps.host => {
                    g.net.requests.push(NetRequest::Host);
                    g.net.status = String::from("Starting the hotspot...");
                    Scene::Online(Screen::Hotspot)
                }
                Some(2) if caps.join => Scene::Online(Screen::JoinWifi {
                    fields: [
                        String::from(HOTSPOT_PREFIX),
                        String::new(),
                        caps.saved_relay.clone(),
                    ],
                    field: 0,
                }),
                Some(3) if caps.join && !caps.saved_ssid.is_empty() => {
                    g.net.requests.push(NetRequest::Join {
                        ssid: caps.saved_ssid.clone(),
                        password: String::new(),
                        relay: caps.saved_relay.clone(),
                    });
                    g.net.status = format!("Joining {}...", caps.saved_ssid);
                    Scene::Online(Screen::Connecting)
                }
                _ if caps.host || caps.join => {
                    if k == Key::Escape {
                        Scene::Title
                    } else {
                        Scene::Online(Screen::Offline)
                    }
                }
                _ => Scene::Title,
            }
        }
        Screen::Hotspot | Screen::Connecting => {
            if k == Key::Escape {
                return Scene::Title;
            }
            if g.net.online && k == Key::Enter {
                return open(g);
            }
            Scene::Online(s)
        }
        Screen::JoinWifi { mut fields, field } => {
            // A Cardputer's hotspot needs no relay address; home Wi-Fi may.
            let last = if fields[0].starts_with(HOTSPOT_PREFIX) {
                1
            } else {
                2
            };
            match k {
                Key::Escape => return Scene::Online(Screen::Offline),
                Key::Backspace => {
                    fields[field].pop();
                }
                Key::Up if field > 0 => {
                    return Scene::Online(Screen::JoinWifi {
                        fields,
                        field: field - 1,
                    });
                }
                Key::Enter if field == 0 && fields[0].trim().is_empty() => {}
                Key::Enter if field < last => {
                    return Scene::Online(Screen::JoinWifi {
                        fields,
                        field: field + 1,
                    });
                }
                Key::Enter => {
                    let [ssid, password, relay] = fields;
                    let ssid = String::from(ssid.trim());
                    g.net.status = format!("Joining {ssid}...");
                    let relay = if last == 2 {
                        String::from(relay.trim())
                    } else {
                        String::new()
                    };
                    g.net.requests.push(NetRequest::Join {
                        ssid,
                        password,
                        relay,
                    });
                    return Scene::Online(Screen::Connecting);
                }
                Key::Char(c) if !c.is_control() && fields[field].chars().count() < 32 => {
                    fields[field].push(c);
                }
                _ => {}
            }
            Scene::Online(Screen::JoinWifi { fields, field })
        }
        Screen::Name(mut buf) => match k {
            Key::Enter => {
                g.net.name = if buf.trim().is_empty() {
                    setup::random_name(g)
                } else {
                    String::from(buf.trim())
                };
                Scene::Online(Screen::Menu)
            }
            Key::Escape => Scene::Title,
            Key::Backspace => {
                buf.pop();
                Scene::Online(Screen::Name(buf))
            }
            Key::Char(c)
                if (c.is_alphanumeric() || c == ' ' || c == '-') && buf.chars().count() < 10 =>
            {
                buf.push(c);
                Scene::Online(Screen::Name(buf))
            }
            _ => Scene::Online(Screen::Name(buf)),
        },
        Screen::Menu => {
            if k == Key::Escape {
                return Scene::Title;
            }
            match ui::digit(k) {
                Some(1) => Scene::Online(Screen::Setup),
                Some(n) if n >= 2 && n - 2 < g.net.lobbies.len() => {
                    let (host, mode, cheats) = {
                        let l = &g.net.lobbies[n - 2];
                        (l.host, l.mode, l.cheats)
                    };
                    g.net.host = Some(host);
                    g.net.settings.mode = mode;
                    g.net.settings.cheats = cheats;
                    g.net.players.clear();
                    let (me, name) = (g.net.me, g.net.name.clone());
                    say(g, "join", &format!("{me} {name}"));
                    g.beep(659, 40);
                    Scene::Online(Screen::Waiting)
                }
                _ if k == Key::Char('n') || k == Key::Char('N') => {
                    Scene::Online(Screen::Name(String::new()))
                }
                _ => Scene::Online(Screen::Menu),
            }
        }
        Screen::Setup => {
            let st = &mut g.net.settings;
            match k {
                Key::Escape => return Scene::Online(Screen::Menu),
                Key::Enter => {
                    g.net.host = Some(g.net.me);
                    g.net.players = alloc::vec![g.net.me_peer()];
                    g.net.advert_in = 0;
                    return Scene::Online(Screen::Waiting);
                }
                _ => {}
            }
            match ui::digit(k) {
                Some(1) => {
                    let i = Mode::ALL.iter().position(|m| *m == st.mode).unwrap_or(0);
                    st.mode = Mode::ALL[(i + 1) % Mode::ALL.len()];
                }
                Some(2) => st.gentle = !st.gentle,
                Some(3) => st.month = st.month % 5 + 1,
                Some(4) => st.cheats = !st.cheats,
                Some(5) => {
                    st.vote_secs = match st.vote_secs {
                        10 => 20,
                        20 => 30,
                        30 => 0,
                        _ => 10,
                    }
                }
                _ => {}
            }
            g.beep(587, 20);
            Scene::Online(Screen::Setup)
        }
        Screen::Waiting => match k {
            Key::Escape => {
                leave(g);
                Scene::Online(Screen::Menu)
            }
            Key::Enter if g.net.hosting() => {
                let seed = g.rng.next_u32() | 1;
                let s = g.net.settings;
                say(
                    g,
                    "start",
                    &format!(
                        "{seed} {} {} {} {}",
                        s.mode.code(),
                        u8::from(s.cheats),
                        u8::from(s.gentle),
                        s.month
                    ),
                );
                begin(g, seed)
            }
            _ => Scene::Online(Screen::Waiting),
        },
        Screen::Standings { last } => match k {
            Key::Char('m' | 'M') if !last => Scene::Online(Screen::Message),
            Key::Char('g' | 'G') if !last && g.net.settings.mode == Mode::Train => {
                Scene::Online(Screen::Give(None))
            }
            _ if last => {
                leave(g);
                Scene::TopTen
            }
            _ => Scene::SizeUp,
        },
        Screen::Message => {
            let watching = crate::party::watching(g);
            if let Some(n) = ui::digit(k).filter(|n| (1..=MESSAGES.len()).contains(n)) {
                let me = g.net.me;
                say(g, "msg", &format!("{me} {}", n - 1));
                g.notify(format!("You: {}", MESSAGES[n - 1]));
                return if watching {
                    Scene::Online(Screen::Remote)
                } else {
                    Scene::SizeUp
                };
            }
            match k {
                Key::Escape if watching => Scene::Online(Screen::Remote),
                Key::Escape => Scene::Online(Screen::Standings { last: false }),
                _ => Scene::Online(Screen::Message),
            }
        }
        Screen::Give(to) => give_key(g, to, k),
        // Keys go to the host while watching (see party::local_key).
        Screen::Remote => Scene::Online(Screen::Remote),
        Screen::RemoteMenu => match ui::digit(k) {
            Some(2) => Scene::Online(Screen::Message),
            Some(3) => {
                leave(g);
                Scene::Online(Screen::Menu)
            }
            _ => Scene::Online(Screen::Remote),
        },
        Screen::HuntResults(r) => match k {
            Key::Enter | Key::Char(' ') => {
                g.net.party.shots.clear();
                hunt_round(g)
            }
            Key::Escape => {
                crate::party::end(g, "The hunt is over. Thanks for hunting!");
                leave(g);
                Scene::Online(Screen::Menu)
            }
            _ => Scene::Online(Screen::HuntResults(r)),
        },
    }
}

fn on_off(b: bool) -> &'static str {
    if b { "allowed" } else { "off" }
}

pub fn render(g: &Game, f: &mut Frame, s: &Screen) {
    let n = &g.net;
    match s {
        Screen::Offline if n.caps.host || n.caps.join => {
            ui::header(f, "Play together");
            let mut y = 18;
            if n.caps.host {
                f.text(
                    6,
                    y,
                    "1. Host a game on this Cardputer",
                    gfx::WHITE,
                    Font::Normal,
                );
                y += 12;
            }
            if n.caps.join {
                f.text(6, y, "2. Join a game on Wi-Fi", gfx::WHITE, Font::Normal);
                y += 12;
                if !n.caps.saved_ssid.is_empty() {
                    f.text(
                        6,
                        y,
                        &format!("3. Join {} again", n.caps.saved_ssid),
                        gfx::WHITE,
                        Font::Normal,
                    );
                    y += 12;
                }
            }
            ui::para(
                f,
                3,
                y + 8,
                "Hosting makes a Wi-Fi hotspot. Phones and computers join it and play in a web browser, no internet needed.",
                gfx::GRAY,
            );
            ui::footer(f, "Pick one.  ESC: back");
        }
        Screen::Hotspot => {
            ui::header(f, "Host a game");
            match &n.host_info {
                None => {
                    f.text(6, 30, &n.status, gfx::YELLOW, Font::Normal);
                }
                Some(h) => {
                    let mut x0 = 4;
                    if h.qr_size > 0 {
                        // Three pixels per module, with a white border for phone cameras.
                        let px = 3;
                        let side = (h.qr_size as i32 + 4) * px;
                        f.rect(2, 14, side, side, gfx::WHITE);
                        for (i, dark) in h.qr.iter().enumerate() {
                            if *dark {
                                let (mx, my) = ((i % h.qr_size) as i32, (i / h.qr_size) as i32);
                                f.rect(2 + (mx + 2) * px, 14 + (my + 2) * px, px, px, gfx::BLACK);
                            }
                        }
                        x0 = side + 8;
                    }
                    let rows = [
                        ("Wi-Fi:", h.ssid.as_str()),
                        ("Password:", h.password.as_str()),
                        ("Then open:", h.address.as_str()),
                    ];
                    let mut y = 16;
                    for (label, value) in rows {
                        f.text(x0, y, label, gfx::GRAY, Font::Small);
                        f.text(x0, y + 9, value, gfx::YELLOW, Font::Normal);
                        y += 24;
                    }
                    f.text(x0, y, "Scan to join with a phone.", gfx::GRAY, Font::Small);
                    // Who has joined so far, or what went wrong.
                    f.text(x0, y + 12, &n.status, gfx::GREEN, Font::Small);
                }
            }
            let footer = if n.online {
                "ENTER: continue  ESC: back"
            } else {
                "ESC: back"
            };
            ui::footer(f, footer);
        }
        Screen::JoinWifi { fields, field } => {
            ui::header(f, "Join a game on Wi-Fi");
            let cursor = if (g.clock / 400) % 2 == 0 { "_" } else { " " };
            let hotspot = fields[0].starts_with(HOTSPOT_PREFIX);
            let labels = ["Wi-Fi name:", "Password:", "Relay address (blank: search):"];
            let shown = if hotspot { 2 } else { 3 };
            for i in 0..shown.min(*field + 1) {
                let y = 16 + i as i32 * 24;
                f.text(6, y, labels[i], gfx::GRAY, Font::Small);
                let c = if i == *field { cursor } else { "" };
                f.text(
                    12,
                    y + 9,
                    &format!("{}{c}", fields[i]),
                    gfx::YELLOW,
                    Font::Normal,
                );
            }
            let help = if hotspot {
                "A Cardputer's game is called WestwardTrail-something."
            } else {
                "On home Wi-Fi, type the address of the computer running trail-relay, or leave it blank to search."
            };
            ui::para(f, 3, 90, help, gfx::GRAY);
            ui::footer(f, "Type, then ENTER.  Up: back a step");
        }
        Screen::Connecting => {
            ui::header(f, "Play together");
            let dots = ".".repeat(((g.clock / 400) % 4) as usize);
            f.text(
                6,
                30,
                &format!("{}{dots}", n.status),
                gfx::YELLOW,
                Font::Normal,
            );
            let footer = if n.online {
                "Connected! ENTER: continue"
            } else {
                "ESC: cancel"
            };
            ui::footer(f, footer);
        }
        Screen::Offline => {
            ui::header(f, "Play together");
            ui::para(
                f,
                3,
                18,
                "To play together, everyone opens the game from the same Westward Trail host: the home relay, or (coming soon) a Cardputer hosting a game. This copy of the game isn't connected to one.",
                gfx::WHITE,
            );
            ui::footer(f, "Press any key.");
        }
        Screen::Name(buf) => {
            ui::header(f, "Play together");
            f.text(3, 20, "What's your name?", gfx::WHITE, Font::Normal);
            let cursor = if (g.clock / 400) % 2 == 0 { "_" } else { " " };
            f.text(12, 40, &format!("{buf}{cursor}"), gfx::YELLOW, Font::Bold);
            ui::footer(f, "Type a name, ENTER. (ENTER alone picks one.)");
        }
        Screen::Menu => {
            ui::header(f, &format!("Play together, {}", n.name));
            f.text(6, 18, "1. Host a new game", gfx::WHITE, Font::Normal);
            if n.lobbies.is_empty() {
                f.text(
                    6,
                    34,
                    "Looking for games to join...",
                    gfx::GRAY,
                    Font::Normal,
                );
            }
            for (i, l) in n.lobbies.iter().take(7).enumerate() {
                f.text(
                    6,
                    34 + i as i32 * 11,
                    &format!(
                        "{}. Join {} - {}, {}/{}",
                        i + 2,
                        l.name,
                        l.mode.name(),
                        l.players,
                        MAX_PLAYERS
                    ),
                    gfx::YELLOW,
                    Font::Normal,
                );
            }
            ui::footer(f, "Number to pick.  N: change name  ESC: back");
        }
        Screen::Setup => {
            ui::header(f, "Host a game");
            let st = &n.settings;
            let mode = String::from(st.mode.name());
            let timer = if st.vote_secs == 0 {
                String::from("no timer")
            } else {
                format!("{} seconds", st.vote_secs)
            };
            let rows = [
                format!("1. Game: {mode}"),
                format!(
                    "2. Pioneers: {}",
                    if st.gentle { "younger" } else { "seasoned" }
                ),
                format!("3. Leave in: {}", setup::MONTH_NAMES[st.month as usize - 1]),
                format!("4. Cheats: {}", on_off(st.cheats)),
                format!("5. Vote timer: {timer}"),
            ];
            for (i, r) in rows.iter().enumerate() {
                let c = if i == 4 && st.mode != Mode::Democracy {
                    gfx::GRAY
                } else {
                    gfx::WHITE
                };
                f.text(6, 18 + i as i32 * 12, r, c, Font::Normal);
            }
            ui::para(f, 6, 80, st.mode.about(), gfx::GRAY);
            f.text(
                6,
                114,
                "Press a number to change it.",
                gfx::GRAY,
                Font::Small,
            );
            ui::footer(f, "ENTER: open the game  ESC: back");
        }
        Screen::Waiting => {
            let host = n.host.map_or_else(String::new, |h| n.name_of(h));
            let title = if n.hosting() {
                String::from("Your game")
            } else {
                format!("{host}'s game")
            };
            ui::header(f, &title);
            let st = &n.settings;
            f.text(
                3,
                15,
                &format!("{}   cheats {}", st.mode.name(), on_off(st.cheats)),
                gfx::GRAY,
                Font::Small,
            );
            for (i, p) in n.players.iter().enumerate() {
                let y = 26 + i as i32 * 11;
                f.rect(8, y + 2, 6, 6, COLORS[i % MAX_PLAYERS]);
                let you = if p.id == n.me { " (you)" } else { "" };
                f.text(20, y, &format!("{}{you}", p.name), gfx::WHITE, Font::Normal);
            }
            if n.players.is_empty() {
                f.text(8, 28, "Asking to join...", gfx::GRAY, Font::Normal);
            }
            let footer = if n.hosting() {
                format!(
                    "ENTER: start ({}/{MAX_PLAYERS})  ESC: close",
                    n.players.len()
                )
            } else {
                String::from("Waiting for the host to start. ESC: leave")
            };
            ui::footer(f, &footer);
        }
        Screen::Standings { last } => {
            ui::header(
                f,
                match (*last, n.settings.mode) {
                    (true, _) => "Race results",
                    (false, Mode::Train) => "The wagon train",
                    _ => "Who's ahead?",
                },
            );
            let mut order: Vec<&Peer> = n.players.iter().collect();
            order.sort_by_key(|p| match p.state {
                PeerState::Done(points) => (0, u32::MAX - points),
                PeerState::Out => (2, u32::MAX - p.miles),
                _ => (1, u32::MAX - p.miles),
            });
            for (i, p) in order.iter().enumerate() {
                let y = 15 + i as i32 * 12;
                f.rect(4, y + 2, 6, 6, n.color(p.id));
                let lost = p.id != n.me && n.clock.wrapping_sub(p.seen) > LOST_MS;
                let status = match p.state {
                    PeerState::Done(points) => format!("in Oregon, {points} pts"),
                    PeerState::Out => String::from("out of the race"),
                    _ if lost => format!("{} mi (lost)", p.miles),
                    _ => format!("{} mi, {} in party", p.miles, p.alive),
                };
                let you = if p.id == n.me { "*" } else { "" };
                f.text(
                    14,
                    y,
                    &format!("{}. {}{you}", i + 1, p.name),
                    gfx::WHITE,
                    Font::Normal,
                );
                f.text(118, y, &status, gfx::YELLOW, Font::Small);
            }
            let footer = if *last {
                "Press any key."
            } else if n.settings.mode == Mode::Train {
                "M: message  G: give supplies  Other: back"
            } else {
                "M: send a message   Any key: back"
            };
            ui::footer(f, footer);
        }
        Screen::Give(None) => {
            ui::header(f, "Give supplies to...");
            let mut y = 16;
            for (i, p) in n.players.iter().filter(|p| p.id != n.me).enumerate() {
                let far = p.miles.abs_diff(g.t.miles);
                let (note, c) = match p.state {
                    PeerState::Done(_) => (String::from("in Oregon"), gfx::GRAY),
                    PeerState::Out => (String::from("trip over"), gfx::GRAY),
                    _ if far <= NEAR => (format!("{far} miles away"), gfx::GREEN),
                    _ => (format!("{far} miles: too far"), gfx::GRAY),
                };
                f.rect(6, y + 2, 6, 6, n.color(p.id));
                f.text(
                    16,
                    y,
                    &format!("{}. {}", i + 1, p.name),
                    gfx::WHITE,
                    Font::Normal,
                );
                f.text(130, y + 1, &note, c, Font::Small);
                y += 12;
            }
            ui::para(
                f,
                6,
                y + 4,
                "Wagons within 50 miles of each other can share. Positions update every few seconds.",
                gfx::GRAY,
            );
            ui::footer(f, "Pick a wagon.  ESC: back");
        }
        Screen::Give(Some(to)) => {
            ui::header(f, &format!("Give to {}", n.name_of(*to)));
            let s = &g.t.sup;
            let have = [s.food, s.oxen, s.ammo, s.clothing, s.wheels, s.axles];
            for (i, (what, _, _)) in GIFTS.iter().enumerate() {
                let y = 16 + i as i32 * 12;
                f.text(
                    6,
                    y,
                    &format!("{}. {what}", i + 1),
                    gfx::WHITE,
                    Font::Normal,
                );
                f.text(
                    150,
                    y + 1,
                    &format!("you have {}", have[i]),
                    gfx::GRAY,
                    Font::Small,
                );
            }
            ui::footer(f, "Pick something to give.  ESC: back");
        }
        Screen::Remote => crate::party::render_remote(g, f),
        Screen::RemoteMenu => {
            ui::header(f, "Your menu");
            ui::menu(
                f,
                6,
                20,
                &["Back to the game", "Send a message", "Leave the game"],
            );
            let host = n.host.map_or_else(String::new, |h| n.name_of(h));
            ui::para(
                f,
                6,
                64,
                &format!(
                    "You're playing {}, hosted by {host}.",
                    n.settings.mode.name()
                ),
                gfx::GRAY,
            );
            ui::footer(f, "Pick one.");
        }
        Screen::HuntResults(results) => {
            ui::header(f, "The hunt is over!");
            let mut order: Vec<&(u32, u32)> = results.iter().collect();
            order.sort_by_key(|(_, lbs)| u32::MAX - lbs);
            let total: u32 = results.iter().map(|(_, lbs)| lbs).sum();
            for (i, (id, lbs)) in order.iter().enumerate() {
                let y = 16 + i as i32 * 12;
                f.rect(6, y + 2, 6, 6, n.color(*id));
                f.text(
                    16,
                    y,
                    &format!("{}. {}", i + 1, n.name_of(*id)),
                    gfx::WHITE,
                    Font::Normal,
                );
                f.text(150, y, &format!("{lbs} lbs"), gfx::YELLOW, Font::Normal);
            }
            f.text(
                6,
                112,
                &format!("Everyone together: {total} pounds of meat"),
                gfx::GREEN,
                Font::Small,
            );
            let footer = if n.hosting() {
                "ENTER: hunt again  ESC: finish"
            } else {
                "Waiting for the host..."
            };
            ui::footer(f, footer);
        }
        Screen::Message => {
            ui::header(f, "Send a message");
            for (i, m) in MESSAGES.iter().enumerate() {
                f.text(
                    6,
                    15 + i as i32 * 12,
                    &format!("{}. {m}", i + 1),
                    gfx::WHITE,
                    Font::Normal,
                );
            }
            ui::footer(f, "Pick one.  ESC: back");
        }
    }
}

/// Little colored wagons for the other racers along a strip on the travel screen.
pub fn render_strip(g: &Game, f: &mut Frame, y: i32) {
    let n = &g.net;
    if !n.racing || n.players.len() < 2 {
        return;
    }
    let len = g.data.trail_length().max(1) as i32;
    f.rect(4, y, W - 8, 1, gfx::rgb(60, 50, 30));
    for p in &n.players {
        let miles = if p.id == n.me { g.t.miles } else { p.miles } as i32;
        // East (the start) on the right, like the wagon heading west.
        let x = W - 8 - (W - 12) * miles.min(len) / len;
        let c = n.color(p.id);
        f.rect(x, y - 2, 4, 4, c);
        if p.id == n.me {
            f.outline(x - 1, y - 3, 6, 6, gfx::WHITE);
        }
    }
}

/// Where each other racer is, for the map.
pub fn others(g: &Game) -> Vec<(u32, Color, String)> {
    let n = &g.net;
    if !n.racing {
        return Vec::new();
    }
    n.players
        .iter()
        .filter(|p| p.id != n.me)
        .map(|p| (p.miles, n.color(p.id), p.name.clone()))
        .collect()
}
