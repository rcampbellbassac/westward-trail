//! The whole game, independent of any screen or keyboard.
//!
//! A platform (web page, Cardputer) does four things in a loop:
//!   1. send keys with [`Game::key_down`] / [`Game::key_up`]
//!   2. call [`Game::tick`] with the milliseconds since last time
//!   3. call [`Game::render`] and show the 240x135 [`gfx::Frame`]
//!   4. play anything returned by [`Game::take_beeps`]
//!
//! Saved trips and the Top Ten come and go as text: hand saved slots in with
//! [`Game::import_slot`] and write whatever [`Game::take_saves`] returns.

#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

pub mod data;
pub mod gfx;
pub mod rng;
pub mod sprites;

mod fish;
mod forage;
mod hunt;
mod net;
mod party;
mod pictures;
mod raft;
mod river;
mod save;
mod score;
mod setup;
mod store;
mod stream;
mod terminal;
mod travel;
mod ui;

use alloc::{boxed::Box, collections::VecDeque, string::String, vec::Vec};

use data::TrailData;
use gfx::{Frame, Theme};
pub use net::{HostInfo, NetRequest};
use rng::Rng;
pub use save::{SLOTS, SaveOp, sequence as save_sequence};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Escape,
    Up,
    Down,
    Left,
    Right,
}

/// One square-wave note. `hz == 0` is a rest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Beep {
    pub hz: u16,
    pub ms: u16,
}

/// Track a key going down or up, for the action mini-games.
pub(crate) fn set_held(h: &mut Held, k: Key, down: bool) {
    match k {
        Key::Up | Key::Char('w' | 'W' | ';') => h.up = down,
        Key::Down | Key::Char('s' | 'S' | '.') => h.down = down,
        Key::Left | Key::Char('a' | 'A' | ',') => h.left = down,
        Key::Right | Key::Char('d' | 'D' | '/') => h.right = down,
        Key::Char(' ') => h.fire = down,
        _ => {}
    }
}

/// Keys currently held down, for the action mini-games.
#[derive(Default, Clone, Copy)]
pub(crate) struct Held {
    pub up: bool,
    pub down: bool,
    pub left: bool,
    pub right: bool,
    pub fire: bool,
}

/// Gentle is for younger pioneers: nobody dies, they go home to rest.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Mood {
    Gentle,
    Classic,
}

pub(crate) struct Person {
    pub name: String,
    /// What they're sick with, and for how many days.
    pub sick: Option<(String, u32)>,
    pub gone: bool,
}

#[derive(Default, Clone)]
pub(crate) struct Supplies {
    /// Money in cents.
    pub cash: u32,
    pub oxen: u32,
    pub food: u32,
    pub clothing: u32,
    /// Individual bullets (they're sold 20 to a box).
    pub ammo: u32,
    pub wheels: u32,
    pub axles: u32,
    pub tongues: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Precip {
    Clear,
    Rain,
    Snow,
}

pub(crate) struct Trip {
    pub mood: Mood,
    pub occupation: usize,
    pub party: Vec<Person>,
    pub sup: Supplies,
    /// Day of the year, 1 = January 1.
    pub day: u32,
    pub days: u32,
    pub miles: u32,
    /// Index of the next landmark ahead.
    pub next: usize,
    /// Landmark we are stopped at, if any.
    pub at: Option<usize>,
    /// 0 steady, 1 strenuous, 2 grueling.
    pub pace: usize,
    /// 0 filling, 1 meager, 2 bare bones.
    pub rations: usize,
    /// Party health points: 0 is perfect, bigger is worse.
    pub health: u32,
    /// Days an ox stays lame.
    pub ox_lame: u32,
    pub temp: i32,
    pub precip: Precip,
}

impl Trip {
    pub fn new() -> Self {
        Trip {
            mood: Mood::Classic,
            occupation: 0,
            party: Vec::new(),
            sup: Supplies::default(),
            day: 60,
            days: 0,
            miles: 0,
            next: 1,
            at: Some(0),
            pace: 0,
            rations: 0,
            health: 0,
            ox_lame: 0,
            temp: 60,
            precip: Precip::Clear,
        }
    }

    pub fn alive(&self) -> impl Iterator<Item = &Person> {
        self.party.iter().filter(|p| !p.gone)
    }

    pub fn alive_count(&self) -> u32 {
        self.alive().count() as u32
    }
}

#[derive(Clone)]
pub(crate) enum Pic {
    None,
    Landmark(usize),
    Tomb(String),
}

pub(crate) struct Msg {
    pub title: String,
    pub body: String,
    pub pic: Pic,
    pub page: usize,
}

/// Toys for hackers. Using any of them marks your score.
pub(crate) struct Cheats {
    pub used: bool,
    pub god: bool,
    pub peek: bool,
    pub infinite_ammo: bool,
    pub speed: u32,
    pub theme: Theme,
}

pub(crate) struct ScoreEntry {
    pub name: String,
    pub points: u32,
}

pub(crate) enum Scene {
    Title,
    Learn(usize),
    TopTen,
    Mood,
    Occupation,
    Names { idx: usize, buf: String },
    ConfirmNames,
    Month,
    Store(store::StoreState),
    Travel { acc_ms: u32 },
    SizeUp,
    Supplies,
    Map,
    Pace,
    Rations,
    Rest { buf: String },
    Trade(Option<travel::Offer>),
    River(river::RiverState),
    Dalles,
    Hunt(Box<hunt::HuntState>),
    Fish(Box<fish::FishState>),
    Forage(Box<forage::ForageState>),
    Raft(Box<raft::RaftState>),
    Score(Vec<(String, u32)>),
    GameOver,
    Terminal(Box<terminal::TermState>),
    Slots(save::SlotState),
    Online(net::Screen),
}

pub struct Game {
    pub(crate) data: TrailData,
    pub(crate) rng: Rng,
    pub(crate) scene: Scene,
    pub(crate) msgs: VecDeque<Msg>,
    pub(crate) beeps: Vec<Beep>,
    pub(crate) held: Held,
    pub(crate) t: Trip,
    pub(crate) cheats: Cheats,
    pub(crate) top: Vec<ScoreEntry>,
    /// Milliseconds since the game started, for blinking things.
    pub(crate) clock: u32,
    /// Saved trips, as text.
    pub(crate) slots: [Option<String>; SLOTS],
    /// Things for the platform to write down.
    pub(crate) saves: Vec<SaveOp>,
    /// A short note shown over the screen ("Saved in slot 2").
    pub(crate) toast: Option<(String, u32)>,
    /// Saves made so far (the biggest "saved = N" seen), so newer saves count higher.
    pub(crate) save_count: u32,
    /// Younger pioneers get the real-life plant safety talk once.
    pub(crate) forage_warned: bool,
    /// Playing together: lobby, race, the lines to send.
    pub(crate) net: net::Net,
    /// A scene the network asked for (the race starting), applied next tick.
    pub(crate) pending: Option<Scene>,
}

impl Game {
    /// A game using the built-in trail data.
    pub fn new(seed: u64) -> Self {
        let data = data::parse(data::DEFAULT_DATA).expect("built-in trail data is valid");
        Self::with_parsed(data, seed)
    }

    /// A game using a modded data file.
    pub fn with_data(src: &str, seed: u64) -> Result<Self, data::DataError> {
        Ok(Self::with_parsed(data::parse(src)?, seed))
    }

    fn with_parsed(data: TrailData, seed: u64) -> Self {
        Game {
            data,
            rng: Rng::new(seed),
            scene: Scene::Title,
            msgs: VecDeque::new(),
            beeps: Vec::new(),
            held: Held::default(),
            t: Trip::new(),
            cheats: Cheats {
                used: false,
                god: false,
                peek: false,
                infinite_ammo: false,
                speed: 1,
                theme: Theme::Color,
            },
            top: score::default_top_ten(),
            clock: 0,
            slots: Default::default(),
            saves: Vec::new(),
            toast: None,
            save_count: 0,
            forage_warned: false,
            net: net::Net::new(seed),
            pending: None,
        }
    }

    pub fn title(&self) -> &str {
        &self.data.title
    }

    pub fn key_down(&mut self, k: Key) {
        // Playing together: the key may belong to someone else's game, or be a vote.
        if party::local_key(self, k, true) {
            return;
        }
        self.press(k);
    }

    pub fn key_up(&mut self, k: Key) {
        if party::local_key(self, k, false) {
            return;
        }
        self.release(k);
    }

    /// Press a key in this game, whoever pressed it.
    pub(crate) fn press(&mut self, k: Key) {
        set_held(&mut self.held, k, true);
        let in_terminal = matches!(self.scene, Scene::Terminal(_));
        if matches!(k, Key::Char('`' | '~')) && !in_terminal && !self.in_action_game() {
            let prev = core::mem::replace(&mut self.scene, Scene::Title);
            self.scene = terminal::open(prev);
            self.beep(880, 30);
            return;
        }
        if !self.msgs.is_empty() && !in_terminal {
            self.advance_msg();
            return;
        }
        let scene = core::mem::replace(&mut self.scene, Scene::Title);
        self.scene = self.on_key(scene, k);
    }

    pub(crate) fn release(&mut self, k: Key) {
        set_held(&mut self.held, k, false);
    }

    pub fn tick(&mut self, ms: u32) {
        let ms = ms.min(250);
        self.clock = self.clock.wrapping_add(ms);
        if let Some((_, t)) = &mut self.toast {
            *t = t.saturating_sub(ms);
            if *t == 0 {
                self.toast = None;
            }
        }
        net::tick(self, ms);
        if !matches!(self.scene, Scene::Terminal(_))
            && let Some(scene) = self.pending.take()
        {
            self.scene = scene;
        }
        if !self.msgs.is_empty() {
            return;
        }
        let scene = core::mem::replace(&mut self.scene, Scene::Title);
        self.scene = self.on_tick(scene, ms);
    }

    pub fn render(&self, f: &mut Frame) {
        f.clear(gfx::BLACK);
        match (self.msgs.front(), &self.scene) {
            (_, Scene::Terminal(t)) => terminal::render(self, f, t),
            (Some(m), _) => ui::render_msg(self, f, m),
            (None, scene) => self.render_scene(f, scene),
        }
        if let Some((text, _)) = &self.toast {
            let w = text.len() as i32 * 6 + 8;
            let x = (gfx::W - w) / 2;
            f.rect(x, 2, w, 13, gfx::rgb(20, 60, 25));
            f.outline(x, 2, w, 13, gfx::GREEN);
            f.text(x + 4, 4, text, gfx::WHITE, gfx::Font::Normal);
        }
        party::overlay(self, f);
        f.apply_theme(self.cheats.theme);
        party::share(self, f);
    }

    /// Notes to play since the last call.
    pub fn take_beeps(&mut self) -> Vec<Beep> {
        core::mem::take(&mut self.beeps)
    }

    /// The Top Ten as text lines ("name|points"), for saving.
    pub fn export_scores(&self) -> String {
        score::export(&self.top)
    }

    pub fn import_scores(&mut self, s: &str) {
        if let Some(top) = score::import(s) {
            self.top = top;
        }
    }

    /// Hand in a saved trip (slot 0-4) that the platform kept. Empty text clears the slot.
    pub fn import_slot(&mut self, slot: usize, text: &str) {
        if slot < SLOTS {
            self.save_count = self.save_count.max(save::sequence(text));
            self.slots[slot] = (!text.trim().is_empty()).then(|| String::from(text));
        }
    }

    /// Saved trips and scores the platform should write down now.
    pub fn take_saves(&mut self) -> Vec<SaveOp> {
        save::take(self)
    }

    /// The platform tells the game whether the relay for playing together is reachable.
    pub fn set_online(&mut self, online: bool) {
        self.net.online = online;
        if !online {
            // A hotspot that stopped is gone: show why instead of its old QR code.
            self.net.host_info = None;
        }
    }

    /// What this platform's network can do (the Cardputer can host and join Wi-Fi games).
    pub fn set_net_caps(&mut self, host: bool, join: bool, saved_ssid: &str, saved_relay: &str) {
        self.net.caps = net::Caps {
            host,
            join,
            saved_ssid: String::from(saved_ssid),
            saved_relay: String::from(saved_relay),
        };
    }

    /// Network jobs for the platform: start a hotspot, join a network.
    pub fn take_net_requests(&mut self) -> Vec<NetRequest> {
        core::mem::take(&mut self.net.requests)
    }

    /// The hotspot this platform is hosting, for the "how to join" screen.
    pub fn set_host_info(&mut self, info: HostInfo) {
        self.net.host_info = Some(info);
    }

    /// What the platform's network is doing, for the waiting screen.
    pub fn set_net_status(&mut self, text: &str) {
        self.net.status = String::from(text);
    }

    /// Lines for the platform to send to the relay.
    pub fn take_net(&mut self) -> Vec<String> {
        self.net.take()
    }

    /// One line that arrived from the relay.
    pub fn net_in(&mut self, line: &str) {
        net::receive(self, line);
    }

    /// Play the title song ("Oh! Susanna") if the title screen is showing.
    /// Hardware calls this at power-on; browsers after the first key press,
    /// since they don't allow sound before that.
    pub fn title_song(&mut self) {
        setup::title_song(self);
    }

    /// Show a short note over the screen for a couple of seconds.
    pub fn notify(&mut self, text: impl Into<String>) {
        let mut text: String = text.into();
        text.truncate(38);
        self.toast = Some((text, 2500));
    }

    // ---- internals ----

    fn in_action_game(&self) -> bool {
        matches!(
            self.scene,
            Scene::Hunt(_) | Scene::Fish(_) | Scene::Raft(_) | Scene::Forage(_)
        )
    }

    pub(crate) fn beep(&mut self, hz: u16, ms: u16) {
        if self.beeps.len() < 64 {
            self.beeps.push(Beep { hz, ms });
        }
    }

    pub(crate) fn tune(&mut self, notes: &[(u16, u16)]) {
        for &(hz, ms) in notes {
            self.beep(hz, ms);
        }
    }

    pub(crate) fn say(&mut self, title: impl Into<String>, body: impl Into<String>) {
        self.msgs.push_back(Msg {
            title: title.into(),
            body: body.into(),
            pic: Pic::None,
            page: 0,
        });
    }

    pub(crate) fn say_pic(&mut self, title: impl Into<String>, body: impl Into<String>, pic: Pic) {
        self.msgs.push_back(Msg {
            title: title.into(),
            body: body.into(),
            pic,
            page: 0,
        });
    }

    fn advance_msg(&mut self) {
        let pages = self.msgs.front().map_or(1, ui::msg_pages);
        if let Some(m) = self.msgs.front_mut()
            && m.page + 1 < pages
        {
            m.page += 1;
            return;
        }
        self.msgs.pop_front();
        self.beep(660, 20);
    }

    fn on_key(&mut self, scene: Scene, k: Key) -> Scene {
        match scene {
            Scene::Title => setup::title_key(self, k),
            Scene::Learn(p) => setup::learn_key(self, p, k),
            Scene::TopTen => Scene::Title,
            Scene::Mood => setup::mood_key(self, k),
            Scene::Occupation => setup::occupation_key(self, k),
            Scene::Names { idx, buf } => setup::names_key(self, idx, buf, k),
            Scene::ConfirmNames => setup::confirm_names_key(self, k),
            Scene::Month => setup::month_key(self, k),
            Scene::Store(s) => store::key(self, s, k),
            Scene::Travel { acc_ms } => travel::travel_key(self, acc_ms, k),
            Scene::SizeUp => travel::sizeup_key(self, k),
            Scene::Supplies | Scene::Map => Scene::SizeUp,
            Scene::Pace => travel::pace_key(self, k),
            Scene::Rations => travel::rations_key(self, k),
            Scene::Rest { buf } => travel::rest_key(self, buf, k),
            Scene::Trade(o) => travel::trade_key(self, o, k),
            Scene::River(r) => river::key(self, r, k),
            Scene::Dalles => river::dalles_key(self, k),
            Scene::Hunt(h) => hunt::key(self, h, k),
            Scene::Fish(s) => fish::key(self, s, k),
            Scene::Forage(s) => forage::key(self, s, k),
            Scene::Raft(r) => Scene::Raft(r),
            Scene::Score(lines) => score::after_score(self, &lines),
            Scene::GameOver => score::after_game_over(self),
            Scene::Terminal(t) => terminal::key(self, t, k),
            Scene::Slots(s) => save::key(self, s, k),
            Scene::Online(s) => net::key(self, s, k),
        }
    }

    fn on_tick(&mut self, scene: Scene, ms: u32) -> Scene {
        match scene {
            Scene::Travel { acc_ms } => travel::travel_tick(self, acc_ms, ms),
            Scene::River(r) => river::tick(self, r, ms),
            Scene::Hunt(h) => hunt::tick(self, h, ms),
            Scene::Fish(s) => fish::tick(self, s, ms),
            Scene::Forage(s) => forage::tick(self, s, ms),
            Scene::Raft(r) => raft::tick(self, r, ms),
            other => other,
        }
    }

    fn render_scene(&self, f: &mut Frame, scene: &Scene) {
        match scene {
            Scene::Title => setup::render_title(self, f),
            Scene::Learn(p) => setup::render_learn(self, f, *p),
            Scene::TopTen => score::render_top_ten(self, f),
            Scene::Mood => setup::render_mood(self, f),
            Scene::Occupation => setup::render_occupation(self, f),
            Scene::Names { idx, buf } => setup::render_names(self, f, *idx, buf),
            Scene::ConfirmNames => setup::render_confirm_names(self, f),
            Scene::Month => setup::render_month(self, f),
            Scene::Store(s) => store::render(self, f, s),
            Scene::Travel { acc_ms } => travel::render_travel(self, f, *acc_ms),
            Scene::SizeUp => travel::render_sizeup(self, f),
            Scene::Supplies => travel::render_supplies(self, f),
            Scene::Map => travel::render_map(self, f),
            Scene::Pace => travel::render_pace(self, f),
            Scene::Rations => travel::render_rations(self, f),
            Scene::Rest { buf } => travel::render_rest(self, f, buf),
            Scene::Trade(o) => travel::render_trade(self, f, o.as_ref()),
            Scene::River(r) => river::render(self, f, r),
            Scene::Dalles => river::render_dalles(self, f),
            Scene::Hunt(h) => hunt::render(self, f, h),
            Scene::Fish(s) => fish::render(self, f, s),
            Scene::Forage(s) => forage::render(self, f, s),
            Scene::Raft(r) => raft::render(self, f, r),
            Scene::Score(lines) => score::render_score(self, f, lines),
            Scene::GameOver => score::render_game_over(self, f),
            Scene::Terminal(_) => {}
            Scene::Slots(s) => save::render(self, f, s),
            Scene::Online(s) => net::render(self, f, s),
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;
    use super::*;

    /// A simple robot player: buys sensible supplies and always presses on.
    fn autoplay(seed: u64, gentle: bool) -> (bool, u32) {
        let mut g = Game::new(seed);
        let mut f = Frame::new();
        let script = [
            Key::Char('1'),
            Key::Char(if gentle { '1' } else { '2' }),
            Key::Char('3'),
            Key::Enter,
            Key::Enter,
            Key::Enter,
            Key::Enter,
            Key::Enter,
            Key::Char('y'),
            Key::Char('2'),
        ];
        for k in script {
            g.key_down(k);
        }
        while !g.msgs.is_empty() {
            g.key_down(Key::Enter);
        }
        for (item, qty) in [
            ('1', "4"),
            ('2', "1000"),
            ('3', "10"),
            ('4', "5"),
            ('5', "1"),
            ('6', "1"),
            ('7', "1"),
        ] {
            g.key_down(Key::Char(item));
            for c in qty.chars() {
                g.key_down(Key::Char(c));
            }
            g.key_down(Key::Enter);
            while !g.msgs.is_empty() {
                g.key_down(Key::Enter);
            }
        }
        g.key_down(Key::Enter);
        for step in 0..200_000u32 {
            if step % 50 == 0 {
                g.render(&mut f);
            }
            if !g.msgs.is_empty() {
                g.key_down(Key::Enter);
                continue;
            }
            match &g.scene {
                Scene::SizeUp => {
                    if g.t.sup.food < 100 && g.t.sup.ammo > 0 && step % 3 == 0 {
                        g.key_down(Key::Char('h'));
                    } else {
                        g.key_down(Key::Char('1'));
                    }
                }
                Scene::Store(_) => g.key_down(Key::Enter),
                Scene::River(r) if r.crossing.is_none() => g.key_down(Key::Char('2')),
                Scene::Dalles => g.key_down(Key::Char('1')),
                Scene::Hunt(_) => {
                    g.key_down(Key::Char(' '));
                    g.key_down(Key::Escape);
                }
                Scene::Score(_) => return (true, g.t.days),
                Scene::GameOver => return (false, g.t.days),
                _ => {}
            }
            g.tick(50);
            let _ = g.take_beeps();
        }
        panic!("seed {seed}: trip never ended");
    }

    #[test]
    fn robot_finishes_many_trips() {
        let mut wins = 0;
        for seed in 0..40 {
            let (won, days) = autoplay(seed, seed % 2 == 0);
            std::println!(
                "seed {seed:2} gentle={} won={won} days={days}",
                seed % 2 == 0
            );
            assert!(days > 60 && days < 400, "seed {seed}: {days} days");
            wins += u32::from(won);
        }
        std::println!("robot reached Oregon {wins}/40");
        assert!(wins >= 10, "only {wins}/40 trips reached Oregon");
    }

    #[test]
    fn mashing_keys_never_panics() {
        let keys = [
            Key::Enter,
            Key::Escape,
            Key::Backspace,
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
            Key::Char(' '),
            Key::Char('`'),
            Key::Char('y'),
            Key::Char('n'),
            Key::Char('h'),
            Key::Char('f'),
            Key::Char('g'),
            Key::Char('s'),
            Key::Char('4'),
            Key::Char('9'),
        ];
        let mut f = Frame::new();
        for seed in 0..20u64 {
            let mut g = Game::new(seed);
            let mut r = rng::Rng::new(seed + 99);
            for _ in 0..30_000 {
                let k = if r.chance(400) {
                    Key::Char(char::from(b'1' + r.below(9) as u8))
                } else {
                    keys[r.below(keys.len() as u32) as usize]
                };
                g.key_down(k);
                if r.chance(300) {
                    g.key_up(k);
                }
                g.tick(r.below(120));
                if r.chance(20) {
                    g.render(&mut f);
                }
            }
        }
    }

    /// Play into the trail, as the robot does, and stop at the first size-up screen.
    fn start_trip(seed: u64) -> Game {
        let mut g = Game::new(seed);
        for k in [Key::Char('1'), Key::Char('2'), Key::Char('3')] {
            g.key_down(k);
        }
        for _ in 0..5 {
            g.key_down(Key::Enter);
        }
        g.key_down(Key::Char('y'));
        g.key_down(Key::Char('2'));
        while !g.msgs.is_empty() {
            g.key_down(Key::Enter);
        }
        for (item, qty) in [('1', "3"), ('2', "500"), ('4', "2")] {
            g.key_down(Key::Char(item));
            for c in qty.chars() {
                g.key_down(Key::Char(c));
            }
            g.key_down(Key::Enter);
            while !g.msgs.is_empty() {
                g.key_down(Key::Enter);
            }
        }
        g.key_down(Key::Enter);
        while !g.msgs.is_empty() {
            g.key_down(Key::Enter);
        }
        assert!(matches!(g.scene, Scene::SizeUp));
        g
    }

    /// `TRAIL_SHOTS=/some/dir cargo test -p trail-core shots` saves screens as BMPs to look at.
    #[test]
    fn shots() {
        let Ok(dir) = std::env::var("TRAIL_SHOTS") else {
            return;
        };
        let mut g = start_trip(7);
        g.t.miles = 700;
        g.t.next = g
            .data
            .landmarks
            .iter()
            .position(|l| l.mile > 700)
            .unwrap_or(1);
        let mut f = Frame::new();
        for (name, scene) in [("map", Scene::Map), ("travel", Scene::Travel { acc_ms: 0 })] {
            g.scene = scene;
            g.render(&mut f);
            std::fs::write(std::format!("{dir}/{name}.bmp"), f.to_bmp()).unwrap();
        }
    }

    #[test]
    fn save_and_load_round_trip() {
        let mut g = start_trip(11);
        g.t.miles = 123;
        g.t.sup.food = 777;
        g.t.party[1].sick = Some((String::from("a fever"), 2));
        g.t.party[2].gone = true;
        g.key_down(Key::Char('s'));
        g.key_down(Key::Char('3'));
        let ops = g.take_saves();
        let SaveOp::Slot(2, text) = &ops[0] else {
            panic!("expected slot 3 to be saved: {ops:?}");
        };
        assert!(save::summary(text).unwrap().contains("Independence"));

        // A fresh game, as after a reboot: the platform hands the slot back.
        let mut h = Game::new(99);
        h.import_slot(2, text);
        h.key_down(Key::Char('4'));
        h.key_down(Key::Char('3'));
        assert!(
            matches!(h.scene, Scene::SizeUp),
            "loading should land on the size-up screen"
        );
        assert_eq!(h.t.miles, 123);
        assert_eq!(h.t.sup.food, 777);
        assert_eq!(h.t.party.len(), g.t.party.len());
        assert_eq!(h.t.party[1].sick, Some((String::from("a fever"), 2)));
        assert!(h.t.party[2].gone);
        assert_eq!(h.rng.state(), g.rng.state());
        assert_eq!(save::to_text(&h), save::to_text(&g));

        // Saving over a used slot asks first.
        g.key_down(Key::Char('s'));
        g.key_down(Key::Char('3'));
        assert!(matches!(g.scene, Scene::Slots(_)));
        g.key_down(Key::Char('n'));
        g.key_down(Key::Escape);
        assert!(matches!(g.scene, Scene::SizeUp));
        assert!(g.take_saves().is_empty());

        // Each save counts higher, so devices without a clock can tell which copy is newest.
        let first = save::sequence(text);
        g.key_down(Key::Char('s'));
        g.key_down(Key::Char('4'));
        let SaveOp::Slot(3, newer) = &g.take_saves()[0] else {
            panic!("expected slot 4");
        };
        assert!(save::sequence(newer) > first);
        // A game that starts from old saves keeps counting from the highest one.
        let mut h = Game::new(1);
        h.import_slot(0, newer);
        assert_eq!(h.save_count, save::sequence(newer));
    }

    #[test]
    fn bad_saves_are_refused_kindly() {
        let g = Game::new(1);
        assert!(save::from_text(&g, "garbage").is_err());
        assert!(
            save::from_text(&g, "trail = Some Other Trail|3|50\nperson = A|here||0\n").is_err()
        );
        let mut h = Game::new(1);
        h.import_slot(0, "food = lots");
        h.key_down(Key::Char('4'));
        h.key_down(Key::Char('1'));
        assert!(!h.msgs.is_empty(), "a broken save should explain itself");
        assert!(matches!(h.scene, Scene::Slots(_)));
    }

    #[test]
    fn screenshots_are_bmp_files() {
        let mut f = Frame::new();
        Game::new(3).render(&mut f);
        let bmp = f.to_bmp();
        assert_eq!(&bmp[..2], b"BM");
        assert_eq!(bmp.len(), 54 + 240 * 3 * 135);
    }

    #[test]
    fn gathering_finds_food_in_season() {
        let mut found = false;
        for seed in 0..10 {
            let mut g = start_trip(seed);
            g.t.day = 160; // early June, near Independence: strawberries, onions...
            g.key_down(Key::Char('g'));
            while !g.msgs.is_empty() {
                g.key_down(Key::Enter);
            }
            assert!(matches!(g.scene, Scene::Forage(_)));
            let food = g.t.sup.food;
            let mut f = Frame::new();
            // Wander in a square, picking as we go.
            for step in 0..400u32 {
                let k = [Key::Right, Key::Down, Key::Left, Key::Up][(step / 25 % 4) as usize];
                g.key_down(k);
                g.tick(60);
                g.key_up(k);
                g.key_down(Key::Char(' '));
                if step % 40 == 0 {
                    g.render(&mut f);
                }
                if !matches!(g.scene, Scene::Forage(_)) {
                    break;
                }
            }
            if matches!(g.scene, Scene::Forage(_)) {
                g.key_down(Key::Escape);
            }
            found |= g.t.sup.food > food;
        }
        assert!(found, "ten trips of gathering in June found nothing");
    }

    #[test]
    fn nothing_grows_in_winter() {
        let mut g = start_trip(4);
        g.t.day = 15; // January
        g.key_down(Key::Char('g'));
        assert!(matches!(g.scene, Scene::SizeUp));
        assert!(!g.msgs.is_empty());
    }

    /// A pretend relay: every line one game sends reaches the others.
    fn pump(games: &mut [&mut Game]) {
        for _ in 0..3 {
            let mut lines: Vec<(usize, String)> = Vec::new();
            for (i, g) in games.iter_mut().enumerate() {
                lines.extend(g.take_net().into_iter().map(|l| (i, l)));
            }
            for (from, l) in lines {
                for (i, g) in games.iter_mut().enumerate() {
                    if i != from {
                        g.net_in(&l);
                    }
                }
            }
        }
    }

    fn type_in(g: &mut Game, s: &str) {
        for c in s.chars() {
            g.key_down(Key::Char(c));
        }
        g.key_down(Key::Enter);
    }

    fn clear_msgs(g: &mut Game) {
        while !g.msgs.is_empty() {
            g.key_down(Key::Enter);
        }
    }

    /// From the start of a race to the first size-up screen.
    fn race_setup(g: &mut Game) {
        clear_msgs(g);
        assert!(
            matches!(g.scene, Scene::Occupation),
            "race should start at occupations"
        );
        g.key_down(Key::Char('1'));
        for _ in 0..5 {
            g.key_down(Key::Enter);
        }
        g.key_down(Key::Char('y'));
        clear_msgs(g);
        assert!(
            matches!(g.scene, Scene::Store(_)),
            "racers skip the month menu"
        );
        for (item, qty) in [('1', "3"), ('2', "800")] {
            g.key_down(Key::Char(item));
            type_in(g, qty);
            clear_msgs(g);
        }
        g.key_down(Key::Enter);
        clear_msgs(g);
        assert!(matches!(g.scene, Scene::SizeUp));
    }

    #[test]
    fn two_players_race() {
        let mut a = Game::new(1);
        let mut b = Game::new(2);
        let mut c = Game::new(3);
        for g in [&mut a, &mut b, &mut c] {
            g.set_online(true);
            g.key_down(Key::Char('5'));
        }
        type_in(&mut a, "Ann");
        type_in(&mut b, "Bob");
        type_in(&mut c, "Cy");
        // Ann hosts a race with cheats off, leaving in May.
        a.key_down(Key::Char('1'));
        a.key_down(Key::Char('4'));
        a.key_down(Key::Char('3'));
        a.key_down(Key::Enter);
        assert!(matches!(a.scene, Scene::Online(net::Screen::Waiting)));
        for g in [&mut a, &mut b, &mut c] {
            g.tick(1600);
        }
        pump(&mut [&mut a, &mut b, &mut c]);
        assert_eq!(b.net.lobbies.len(), 1, "Bob should see Ann's game");
        assert_eq!(b.net.lobbies[0].name, "Ann");
        b.key_down(Key::Char('2'));
        c.key_down(Key::Char('2'));
        pump(&mut [&mut a, &mut b, &mut c]);
        assert_eq!(a.net.players.len(), 3);
        assert_eq!(b.net.players.len(), 3, "Bob should get the whole roster");

        // Cy changes their mind and leaves before the start.
        c.key_down(Key::Escape);
        pump(&mut [&mut a, &mut b, &mut c]);
        assert_eq!(a.net.players.len(), 2);

        a.key_down(Key::Enter);
        pump(&mut [&mut a, &mut b, &mut c]);
        b.tick(16);
        assert!(a.net.racing && b.net.racing && !c.net.racing);
        assert_eq!(a.rng.state(), b.rng.state(), "racers share the same luck");
        assert!(!b.net.settings.cheats);

        race_setup(&mut a);
        race_setup(&mut b);
        assert_eq!(a.t.party[0].name, "Ann", "the leader is the player");
        assert_eq!(a.t.day, b.t.day, "everyone leaves in the host's month");
        assert_eq!(ui::month_index(&a), 4, "May");

        // Cheats are off: the terminal refuses.
        b.key_down(Key::Char('`'));
        type_in(&mut b, "give food 9999");
        b.key_down(Key::Escape);
        assert!(b.t.sup.food < 9000);
        assert!(!b.cheats.used);

        // Ann drives ahead; Bob's standings see it.
        a.key_down(Key::Char('1'));
        for _ in 0..400 {
            a.tick(100);
            b.tick(100);
            clear_msgs(&mut a);
            if matches!(a.scene, Scene::SizeUp) {
                a.key_down(Key::Char('1'));
            }
            pump(&mut [&mut a, &mut b]);
        }
        let ann = b.net.players.iter().find(|p| p.name == "Ann").unwrap();
        assert!(ann.miles > 0, "Bob should see Ann's progress");
        b.key_down(Key::Char('r'));
        assert!(matches!(
            b.scene,
            Scene::Online(net::Screen::Standings { .. })
        ));
        let mut f = Frame::new();
        b.render(&mut f);
        b.key_down(Key::Char('m'));
        b.key_down(Key::Char('1'));
        pump(&mut [&mut a, &mut b]);
        assert!(
            a.toast
                .as_ref()
                .is_some_and(|(t, _)| t.contains("Race you"))
        );
    }

    fn pump_all(games: &mut [Game]) {
        let mut refs: Vec<&mut Game> = games.iter_mut().collect();
        pump(&mut refs);
    }

    /// Ann hosts a game of the mode `steps` presses along the list, everyone
    /// else joins, and Ann starts it.
    fn together(steps: usize, names: &[&str]) -> Vec<Game> {
        let mut games: Vec<Game> = (0..names.len()).map(|i| Game::new(10 + i as u64)).collect();
        for (g, name) in games.iter_mut().zip(names) {
            g.set_online(true);
            g.key_down(Key::Char('5'));
            type_in(g, name);
        }
        games[0].key_down(Key::Char('1'));
        for _ in 0..steps {
            games[0].key_down(Key::Char('1'));
        }
        games[0].key_down(Key::Enter);
        for g in games.iter_mut() {
            g.tick(1600);
        }
        pump_all(&mut games);
        for g in games.iter_mut().skip(1) {
            g.key_down(Key::Char('2'));
        }
        pump_all(&mut games);
        assert_eq!(games[0].net.players.len(), names.len());
        games[0].key_down(Key::Enter);
        pump_all(&mut games);
        for g in games.iter_mut() {
            g.tick(16);
        }
        games
    }

    /// The host draws, its screen goes out, and the watchers draw it too.
    fn share_screens(games: &mut [Game]) -> Frame {
        let mut f = Frame::new();
        games[0].net.party.tx.get_mut().tick(1000);
        games[0].render(&mut f);
        pump_all(games);
        f
    }

    #[test]
    fn wagon_train_gives_supplies() {
        let mut games = together(1, &["Ann", "Bob"]);
        assert_eq!(games[0].net.settings.mode, net::Mode::Train);
        for g in games.iter_mut() {
            race_setup(g);
        }
        let food = games[0].t.sup.food;
        let b = &mut games[1];
        b.key_down(Key::Char('r'));
        b.key_down(Key::Char('g'));
        assert!(matches!(b.scene, Scene::Online(net::Screen::Give(None))));
        b.key_down(Key::Char('1'));
        b.key_down(Key::Char('1'));
        let mut f = Frame::new();
        b.render(&mut f);
        pump_all(&mut games);
        assert_eq!(games[0].t.sup.food, food + 50, "Ann gets Bob's food");
        assert_eq!(games[1].t.sup.food, 750);
    }

    #[test]
    fn one_wagon_many_drivers() {
        let mut games = together(3, &["Ann", "Bob"]);
        assert_eq!(games[0].net.settings.mode, net::Mode::Shared);
        assert!(matches!(games[1].scene, Scene::Online(net::Screen::Remote)));
        assert!(matches!(games[0].scene, Scene::Occupation));

        // Bob sees exactly Ann's screen.
        let f = share_screens(&mut games);
        let mut seen = Frame::new();
        games[1].toast = None; // Bob's own note ("press ` for your menu")
        games[1].render(&mut seen);
        assert!(seen.px == f.px, "the watcher's screen matches the host's");

        // Bob isn't driving yet: his keys don't count.
        games[1].key_down(Key::Enter);
        pump_all(&mut games);
        assert!(!games[0].msgs.is_empty());

        // Ann sets up; party members are the players.
        race_setup(&mut games[0]);
        assert_eq!(games[0].t.party[1].name, "Bob");

        // Setting off passes the turn to Bob.
        games[0].key_down(Key::Char('1'));
        games[0].tick(16);
        assert_eq!(games[0].net.party.driver, games[1].net.me);
        pump_all(&mut games);
        assert_eq!(games[1].net.party.driver, games[1].net.me);
        // Drive on to the next choice: a landmark or the first river.
        for _ in 0..3000 {
            games[0].tick(100);
            if !games[0].msgs.is_empty() {
                games[1].key_down(Key::Enter);
                games[1].key_up(Key::Enter);
                pump_all(&mut games);
            } else if matches!(games[0].scene, Scene::SizeUp | Scene::River(_)) {
                break;
            }
            let _ = games[0].take_net();
        }
        let at_river = matches!(games[0].scene, Scene::River(_));
        let choice = |g: &Game| match &g.scene {
            Scene::River(r) => r.crossing.is_none() && g.msgs.is_empty(),
            Scene::SizeUp => g.msgs.is_empty(),
            _ => false,
        };
        assert!(choice(&games[0]), "should be at a choice");
        let pick = if at_river { '3' } else { '2' };
        games[0].key_down(Key::Char(pick));
        assert!(choice(&games[0]), "Ann isn't driving");
        games[1].key_down(Key::Char(pick));
        pump_all(&mut games);
        assert!(!choice(&games[0]), "Bob is");
    }

    #[test]
    fn democracy_votes() {
        let mut games = together(2, &["Ann", "Bob", "Cy"]);
        assert_eq!(games[0].net.settings.mode, net::Mode::Democracy);
        clear_msgs(&mut games[0]);
        // Everyone votes on who to be: two votes for banker wins.
        games[0].key_down(Key::Char('1'));
        assert!(
            matches!(games[0].scene, Scene::Occupation),
            "one vote isn't enough"
        );
        games[1].key_down(Key::Char('2'));
        games[2].key_down(Key::Char('2'));
        pump_all(&mut games);
        assert!(matches!(games[0].scene, Scene::Names { .. }));
        assert_eq!(games[0].t.occupation, 1);

        // The leader types the names and shops.
        for _ in 0..5 {
            games[0].key_down(Key::Enter);
        }
        games[0].key_down(Key::Char('y'));
        clear_msgs(&mut games[0]);
        for (item, qty) in [('1', "3"), ('2', "800")] {
            games[0].key_down(Key::Char(item));
            type_in(&mut games[0], qty);
            clear_msgs(&mut games[0]);
        }
        games[0].key_down(Key::Enter);
        clear_msgs(&mut games[0]);
        assert!(matches!(games[0].scene, Scene::SizeUp));
        assert_eq!(games[0].t.party[2].name, "Cy");

        // Everyone picks "change pace".
        for g in games.iter_mut() {
            g.key_down(Key::Char('4'));
        }
        pump_all(&mut games);
        assert!(matches!(games[0].scene, Scene::Pace));

        // Cy's party member has gone home, so Cy is a ghost; a tie goes to the leader.
        games[0].t.party[2].gone = true;
        games[0].key_down(Key::Char('3'));
        games[1].key_down(Key::Char('2'));
        games[2].key_down(Key::Char('1'));
        pump_all(&mut games);
        assert!(matches!(games[0].scene, Scene::SizeUp));
        assert_eq!(games[0].t.pace, 2, "the leader's vote breaks the tie");

        // Only Bob votes, and the timer decides.
        games[1].key_down(Key::Char('2'));
        pump_all(&mut games);
        let f = share_screens(&mut games);
        let _ = f;
        for _ in 0..40 {
            games[0].tick(250);
        }
        assert!(matches!(games[0].scene, Scene::SizeUp), "10 seconds in");
        for _ in 0..41 {
            games[0].tick(250);
        }
        assert!(
            matches!(games[0].scene, Scene::Supplies),
            "20 seconds: time's up"
        );
    }

    #[test]
    fn hunting_together() {
        let mut games = together(4, &["Ann", "Bob"]);
        assert_eq!(games[0].net.settings.mode, net::Mode::Hunt);
        let start = match &games[0].scene {
            Scene::Hunt(h) => {
                assert!(h.together());
                h.spots()
            }
            _ => panic!("the host should be hunting"),
        };
        assert_eq!(start.len(), 2);
        // Typing "Ann" left the A key (walk left) down; let go of it.
        games[0].held = Held::default();
        // Bob walks right.
        games[1].key_down(Key::Right);
        pump_all(&mut games);
        games[0].tick(200);
        games[1].key_up(Key::Right);
        pump_all(&mut games);
        games[0].tick(200);
        let Scene::Hunt(h) = &games[0].scene else {
            panic!("still hunting");
        };
        let now = h.spots();
        assert_eq!(now[0], start[0], "Ann didn't move");
        assert!(now[1].0 > start[1].0, "Bob walked right");
        let _ = share_screens(&mut games);

        // Time runs out; everyone sees the results; Ann finishes.
        for _ in 0..140 {
            games[0].tick(250);
        }
        assert!(matches!(
            games[0].scene,
            Scene::Online(net::Screen::HuntResults(_))
        ));
        clear_msgs(&mut games[0]);
        games[0].key_down(Key::Escape);
        pump_all(&mut games);
        games[1].tick(16);
        assert!(!games[1].net.racing);
        assert!(matches!(games[1].scene, Scene::Online(net::Screen::Menu)));
    }

    #[test]
    fn offline_play_together_explains() {
        let mut g = Game::new(1);
        g.key_down(Key::Char('5'));
        assert!(matches!(g.scene, Scene::Online(net::Screen::Offline)));
        assert!(g.take_net().is_empty());
    }

    #[test]
    fn terminal_commands_never_panic() {
        let mut g = Game::new(5);
        g.key_down(Key::Char('`'));
        for cmd in [
            "help",
            "give food 99999999",
            "give cash 4294967295",
            "give parts 5",
            "heal",
            "god",
            "ammo",
            "speed 99",
            "warp 5000",
            "win",
            "weather snow",
            "theme green",
            "theme color",
            "peek",
            "seed 7",
            "ls",
            "cat trail.txt",
            "cat secrets",
            "whoami",
            "matrix",
            "forage",
            "sudo give oxen 3",
            "nonsense",
            "clear",
        ] {
            for c in cmd.chars() {
                g.key_down(Key::Char(c));
            }
            g.key_down(Key::Enter);
            if !matches!(g.scene, Scene::Terminal(_)) {
                g.key_down(Key::Char('`'));
            }
        }
        let mut f = Frame::new();
        g.render(&mut f);
    }
}
