//! The whole game, independent of any screen or keyboard.
//!
//! A platform (web page, Cardputer) does four things in a loop:
//!   1. send keys with [`Game::key_down`] / [`Game::key_up`]
//!   2. call [`Game::tick`] with the milliseconds since last time
//!   3. call [`Game::render`] and show the 240x135 [`gfx::Frame`]
//!   4. play anything returned by [`Game::take_beeps`]

#![no_std]
#![forbid(unsafe_code)]
extern crate alloc;

pub mod data;
pub mod gfx;
pub mod rng;
pub mod sprites;

mod fish;
mod hunt;
mod pictures;
mod raft;
mod river;
mod score;
mod setup;
mod store;
mod terminal;
mod travel;
mod ui;

use alloc::{boxed::Box, collections::VecDeque, string::String, vec::Vec};

use data::TrailData;
use gfx::{Frame, Theme};
use rng::Rng;

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
    Raft(Box<raft::RaftState>),
    Score(Vec<(String, u32)>),
    GameOver,
    Terminal(Box<terminal::TermState>),
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
        }
    }

    pub fn title(&self) -> &str {
        &self.data.title
    }

    pub fn key_down(&mut self, k: Key) {
        self.set_held(k, true);
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

    pub fn key_up(&mut self, k: Key) {
        self.set_held(k, false);
    }

    pub fn tick(&mut self, ms: u32) {
        let ms = ms.min(250);
        self.clock = self.clock.wrapping_add(ms);
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
        f.apply_theme(self.cheats.theme);
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

    // ---- internals ----

    fn in_action_game(&self) -> bool {
        matches!(self.scene, Scene::Hunt(_) | Scene::Fish(_) | Scene::Raft(_))
    }

    fn set_held(&mut self, k: Key, down: bool) {
        let h = &mut self.held;
        match k {
            Key::Up | Key::Char('w' | 'W' | ';') => h.up = down,
            Key::Down | Key::Char('s' | 'S' | '.') => h.down = down,
            Key::Left | Key::Char('a' | 'A' | ',') => h.left = down,
            Key::Right | Key::Char('d' | 'D' | '/') => h.right = down,
            Key::Char(' ') => h.fire = down,
            _ => {}
        }
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
            Scene::Raft(r) => Scene::Raft(r),
            Scene::Score(_) => score::after_score(self),
            Scene::GameOver => Scene::Title,
            Scene::Terminal(t) => terminal::key(self, t, k),
        }
    }

    fn on_tick(&mut self, scene: Scene, ms: u32) -> Scene {
        match scene {
            Scene::Travel { acc_ms } => travel::travel_tick(self, acc_ms, ms),
            Scene::River(r) => river::tick(self, r, ms),
            Scene::Hunt(h) => hunt::tick(self, h, ms),
            Scene::Fish(s) => fish::tick(self, s, ms),
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
            Scene::Raft(r) => raft::render(self, f, r),
            Scene::Score(lines) => score::render_score(self, f, lines),
            Scene::GameOver => score::render_game_over(self, f),
            Scene::Terminal(_) => {}
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
