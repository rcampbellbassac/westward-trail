//! Robot players with different skills, to check that each mode is as hard as it should be:
//! younger pioneers winnable for a young kid, seasoned harder but doable, survival brutal.
//!
//! `cargo test --release -p trail-core playtest -- --nocapture --ignored` prints a table.

extern crate std;

use alloc::{collections::VecDeque, string::String, vec::Vec};
use std::{collections::BTreeMap, println};

use crate::{
    Game, Key, Scene,
    data::Kind,
    river::Opt,
    rng::Rng,
    travel::{self, Act},
};

/// How a robot plays.
#[derive(Clone, Copy)]
pub(crate) struct Persona {
    pub name: &'static str,
    /// Title-screen choice on the "who is going west" menu: '1' younger, '2' seasoned, '3' survival.
    pub mood: char,
    /// Buys what the store keeper recommends (otherwise buys at random, like a young kid).
    pub follows_advice: bool,
    /// Leaves in April or May (otherwise any month).
    pub good_month: bool,
    /// Rests when health is poor, stops the wagon when things go wrong.
    pub careful: bool,
    /// Picks safe river crossings (otherwise always the first choice: ford).
    pub smart_rivers: bool,
    /// Buys food and parts at forts.
    pub shops: bool,
    /// Out of 1000: how well the robot aims and steers.
    pub skill: u32,
}

pub(crate) const KID: Persona = Persona {
    name: "young kid, younger",
    mood: '1',
    follows_advice: false,
    good_month: false,
    careful: false,
    smart_rivers: false,
    shops: false,
    skill: 350,
};
pub(crate) const KID_HELPED: Persona = Persona {
    name: "kid who reads, younger",
    mood: '1',
    follows_advice: true,
    good_month: false,
    careful: false,
    smart_rivers: false,
    shops: true,
    skill: 500,
};
pub(crate) const KID_SEASONED: Persona = Persona {
    name: "kid who reads, seasoned",
    mood: '2',
    ..KID_HELPED
};
pub(crate) const ADULT: Persona = Persona {
    name: "careful adult, seasoned",
    mood: '2',
    follows_advice: true,
    good_month: true,
    careful: true,
    smart_rivers: true,
    shops: true,
    skill: 750,
};
pub(crate) const ADULT_SURVIVAL: Persona = Persona {
    name: "careful adult, survival",
    mood: '3',
    ..ADULT
};
pub(crate) const KID_SURVIVAL: Persona = Persona {
    name: "kid who reads, survival",
    mood: '3',
    ..KID_HELPED
};

#[derive(Default, Debug)]
pub(crate) struct Outcome {
    pub won: bool,
    pub stuck: bool,
    pub days: u32,
    pub survivors: u32,
    pub losses: Vec<String>,
}

struct Bot {
    p: Persona,
    r: Rng,
    keys: VecDeque<Key>,
    shopped_at: Option<usize>,
    hunts_here: u32,
    rests_here: u32,
    stop_cool: u32,
    last_at: Option<usize>,
    raft_dir: i32,
}

impl Bot {
    fn press(&mut self, g: &mut Game, k: Key) {
        g.key_down(k);
        g.key_up(k);
    }

    fn type_qty(&mut self, item: usize, qty: u32) {
        self.keys
            .push_back(Key::Char(char::from(b'0' + item as u8)));
        for c in alloc::format!("{qty}").chars() {
            self.keys.push_back(Key::Char(c));
        }
        self.keys.push_back(Key::Enter);
    }

    /// Store choices (1-7: oxen yokes, food lbs, clothing, ammo boxes, wheel, axle, tongue).
    fn shopping_list(&mut self, g: &Game, first: bool) -> Vec<(usize, u32)> {
        let people = g.t.alive_count().max(1);
        if !self.p.follows_advice {
            // A young kid: a bit of everything, in amounts that look nice.
            return alloc::vec![
                (1, self.r.range(1, 3) as u32),
                (2, [100, 200, 300, 500][self.r.below(4) as usize]),
                (3, self.r.range(0, 4) as u32),
                (4, self.r.range(0, 4) as u32),
            ];
        }
        let s = &g.t.sup;
        let mut v = Vec::new();
        if first {
            v.push((1, 3));
            v.push((2, 200 * people));
            v.push((3, 2 * people));
            v.push((4, 4));
            v.push((5, 1));
            v.push((6, 1));
            v.push((7, 1));
        } else {
            let food_goal = 150 * people;
            if s.food < food_goal {
                v.push((2, food_goal - s.food));
            }
            if s.oxen < 4 {
                v.push((1, 1));
            }
            if s.clothing < 2 * people {
                v.push((3, 2 * people - s.clothing));
            }
            if s.wheels == 0 {
                v.push((5, 1));
            }
            if s.axles == 0 {
                v.push((6, 1));
            }
            if s.tongues == 0 {
                v.push((7, 1));
            }
            if s.ammo < 40 {
                v.push((4, 2));
            }
        }
        v
    }

    fn go_shopping(&mut self, g: &Game, first: bool) {
        // Too much at once fails with a message; the robot halves and tries again.
        for (item, qty) in self.shopping_list(g, first) {
            let mut q = qty;
            for _ in 0..3 {
                self.type_qty(item, q);
                q = (q / 2).max(1);
            }
        }
        self.keys.push_back(Key::Enter);
    }

    fn sizeup(&mut self, g: &mut Game) {
        if self.last_at != g.t.at {
            self.last_at = g.t.at;
            self.hunts_here = 0;
            self.rests_here = 0;
        }
        let acts = travel::actions(g);
        let digit = |a: Act| {
            acts.iter()
                .position(|(x, _)| *x == a)
                .map(|i| Key::Char(char::from(b'1' + i as u8)))
        };
        let here = g.t.at;
        let people = g.t.alive_count().max(1);
        // Shop once per fort.
        if self.p.shops
            && here.is_some()
            && self.shopped_at != here
            && let Some(k) = digit(Act::Buy)
        {
            self.shopped_at = here;
            self.keys.push_back(k);
            self.go_shopping(g, false);
            return;
        }
        let stuck = g.t.sup.oxen == 0;
        if (self.p.careful && g.t.health >= 70 && self.rests_here < 2) || stuck {
            self.rests_here += 1;
            if let Some(k) = digit(Act::Rest) {
                self.keys.extend([k, Key::Char('4'), Key::Enter]);
                return;
            }
        }
        let low_food = g.t.sup.food < people * 3 * 12;
        let wants_hunt = if self.p.careful {
            low_food && self.hunts_here < 3
        } else {
            // Kids love hunting, and hunt when they're hungry, too.
            self.hunts_here < 2 && (self.r.chance(120) || (low_food && self.r.chance(500)))
        };
        if wants_hunt && g.t.sup.ammo > 0 {
            self.hunts_here += 1;
            self.keys.push_back(Key::Char('h'));
            return;
        }
        if self.p.careful && g.t.sup.food == 0 && g.t.sup.ammo == 0 {
            // Nothing to eat and nothing to hunt with: try to trade.
            if self.hunts_here < 4
                && let Some(k) = digit(Act::Trade)
            {
                self.hunts_here += 1;
                self.keys.extend([k, Key::Char('y'), Key::Enter]);
                return;
            }
        }
        self.keys.push_back(Key::Char('1'));
    }

    fn river(&mut self, g: &Game, r: &crate::river::RiverState) {
        let opts = crate::river::options(g, r);
        let pick = |want: &dyn Fn(&Opt) -> bool| opts.iter().position(|(o, _)| want(o));
        let i = if !self.p.smart_rivers {
            0
        } else if r.depth10 < 25 {
            pick(&|o| matches!(o, Opt::Ford)).unwrap_or(0)
        } else if let Some(i) = pick(&|o| matches!(o, Opt::Ferry(c) if g.t.sup.cash >= *c * 100)) {
            i
        } else if let Some(i) = pick(&|o| matches!(o, Opt::Guide)).filter(|_| g.t.sup.clothing >= 6)
        {
            i
        } else if r.depth10 > 50 && self.hunts_here < 3 {
            self.hunts_here += 1;
            pick(&|o| matches!(o, Opt::Wait)).unwrap_or(1)
        } else {
            pick(&|o| matches!(o, Opt::Float)).unwrap_or(1)
        };
        self.keys.push_back(Key::Char(char::from(b'1' + i as u8)));
    }

    fn hunt(&mut self, g: &mut Game, view: Option<(i32, i32)>) {
        for k in [Key::Left, Key::Right, Key::Up, Key::Down] {
            g.key_up(k);
        }
        let Some((dx, dy)) = view else {
            return;
        };
        if !self.r.chance(self.p.skill + 200) {
            return; // not paying attention
        }
        let sx = if dx.abs() > 10 { dx.signum() } else { 0 };
        let sy = if dy.abs() > 6 { dy.signum() } else { 0 };
        // Walk toward the animal; walking also turns the hunter to face it.
        if dx.abs() > 40 || dy.abs() > 6 {
            if sx < 0 {
                g.key_down(Key::Left);
            }
            if sx > 0 {
                g.key_down(Key::Right);
            }
            if sy < 0 {
                g.key_down(Key::Up);
            }
            if sy > 0 {
                g.key_down(Key::Down);
            }
        }
        let lined_up = dy.abs() <= 5 && dx.abs() < 110;
        let wild = self.r.chance((1000 - self.p.skill) / 20);
        if (lined_up && self.r.chance(self.p.skill / 3)) || wild {
            self.press(g, Key::Char(' '));
        }
    }

    fn raft(&mut self, g: &mut Game, (x, target): (i32, i32)) {
        if self.r.chance(self.p.skill) {
            self.raft_dir = (target - x).signum() * i32::from((target - x).abs() > 1);
        }
        g.key_up(Key::Left);
        g.key_up(Key::Right);
        match self.raft_dir {
            -1 => g.key_down(Key::Left),
            1 => g.key_down(Key::Right),
            _ => {}
        }
    }
}

pub(crate) fn play(p: Persona, seed: u64) -> Outcome {
    let mut g = Game::new(seed);
    let mut bot = Bot {
        p,
        r: Rng::new(seed ^ 0x5eed),
        keys: VecDeque::new(),
        shopped_at: None,
        hunts_here: 0,
        rests_here: 0,
        stop_cool: 0,
        last_at: None,
        raft_dir: 0,
    };
    let occupation = char::from(b'1' + bot.r.below(3) as u8);
    let month = if p.good_month {
        char::from(b'2' + bot.r.below(2) as u8)
    } else {
        char::from(b'1' + bot.r.below(5) as u8)
    };
    bot.keys
        .extend([Key::Char('1'), Key::Char(p.mood), Key::Char(occupation)]);
    bot.keys.extend([Key::Enter; 5]);
    bot.keys.extend([Key::Char('y'), Key::Char(month)]);
    let mut out = Outcome::default();
    let mut shopped = false;
    // PLAYTEST_SHOTS=dir saves a picture whenever a new screen or message shows.
    let shots = std::env::var("PLAYTEST_SHOTS").ok();
    let mut last_look = String::new();
    let mut shot_n = 0;
    let mut frame = crate::gfx::Frame::new();
    for _ in 0..600_000u32 {
        if matches!(g.scene, Scene::GameOver) || (shopped && matches!(g.scene, Scene::Title)) {
            break;
        }
        if let Some(dir) = &shots {
            let look = match g.msgs.front() {
                Some(m) => alloc::format!("{}|{}|{}", m.title, m.body, m.page),
                None => alloc::format!("{:?}", core::mem::discriminant(&g.scene)),
            };
            if look != last_look && shot_n < 400 {
                last_look = look;
                g.render(&mut frame);
                std::fs::write(alloc::format!("{dir}/{shot_n:03}.bmp"), frame.to_bmp()).unwrap();
                shot_n += 1;
            }
        }
        if let Some(m) = g.msgs.front() {
            if m.body.contains("has died of") || m.title == "Going home" {
                let cause = m
                    .body
                    .split("has died of ")
                    .nth(1)
                    .map_or("went home", |c| c.trim_end_matches('.'));
                out.losses.push(String::from(cause));
            }
            bot.press(&mut g, Key::Enter);
            continue;
        }
        if let Some(k) = bot.keys.pop_front() {
            bot.press(&mut g, k);
            continue;
        }
        match &g.scene {
            Scene::Store(_) if !shopped => {
                shopped = true;
                bot.go_shopping(&g, true);
            }
            Scene::Store(_) => bot.keys.push_back(Key::Enter),
            Scene::SizeUp => bot.sizeup(&mut g),
            Scene::Travel { .. } => {
                bot.stop_cool = bot.stop_cool.saturating_sub(1);
                let trouble = g.t.health >= 70 || g.t.sup.food < g.t.alive_count() * 3 * 5;
                if p.careful && trouble && bot.stop_cool == 0 {
                    bot.stop_cool = 200;
                    bot.last_at = Some(usize::MAX);
                    bot.keys.push_back(Key::Enter);
                }
            }
            Scene::River(r) if r.crossing.is_none() => {
                let r = crate::river::RiverState {
                    idx: r.idx,
                    depth10: r.depth10,
                    width: r.width,
                    crossing: None,
                };
                bot.river(&g, &r);
            }
            Scene::Dalles => {
                let toll = g.t.at.map_or(30, |i| g.data.landmarks[i].toll);
                let road = p.careful && g.t.sup.cash >= toll * 100;
                bot.keys.push_back(Key::Char(if road { '2' } else { '1' }));
            }
            Scene::Hunt(h) => {
                let view = h.bot_view();
                bot.hunt(&mut g, view);
            }
            Scene::Raft(s) => {
                let t = s.bot_target();
                bot.raft(&mut g, t);
            }
            Scene::Fish(_) | Scene::Forage(_) | Scene::Games(_) | Scene::Rest { .. } => {
                bot.keys.push_back(Key::Escape);
            }
            Scene::Trade(_) => bot.keys.push_back(Key::Char('n')),
            Scene::Score(_) => {
                out.won = true;
                break;
            }
            Scene::GameOver => break,
            _ => {}
        }
        g.tick(50);
        let _ = g.take_beeps();
    }
    if !out.won && !matches!(g.scene, Scene::GameOver | Scene::Title) {
        out.stuck = true;
        if std::env::var("PLAYTEST_DEBUG").is_ok() {
            let scene = match &g.scene {
                Scene::SizeUp => "sizeup",
                Scene::Travel { .. } => "travel",
                Scene::Store(_) => "store",
                Scene::River(_) => "river",
                Scene::Trade(_) => "trade",
                Scene::Rest { .. } => "rest",
                Scene::Hunt(_) => "hunt",
                Scene::Raft(_) => "raft",
                Scene::Dalles => "dalles",
                Scene::Fish(_) => "fish",
                Scene::Score(_) => "score",
                Scene::Title => "title",
                Scene::TopTen => "topten",
                Scene::Map => "map",
                Scene::Supplies => "supplies",
                _ => "other",
            };
            println!(
                "stuck seed {seed}: {scene} day {} miles {} oxen {} food {} ammo {} cash {} health {} alive {}",
                g.t.days,
                g.t.miles,
                g.t.sup.oxen,
                g.t.sup.food,
                g.t.sup.ammo,
                g.t.sup.cash,
                g.t.health,
                g.t.alive_count()
            );
        }
    }
    out.days = g.t.days;
    out.survivors = g.t.alive_count();
    let _ = Kind::End;
    out
}

pub(crate) struct Stats {
    pub trips: u32,
    pub wins: u32,
    pub stuck: u32,
    pub survivors: u32,
    pub days: u32,
    pub losses: BTreeMap<String, u32>,
}

pub(crate) fn run(p: Persona, seeds: core::ops::Range<u64>) -> Stats {
    let mut s = Stats {
        trips: 0,
        wins: 0,
        stuck: 0,
        survivors: 0,
        days: 0,
        losses: BTreeMap::new(),
    };
    for seed in seeds {
        let o = play(p, seed);
        s.trips += 1;
        s.wins += u32::from(o.won);
        s.stuck += u32::from(o.stuck);
        if o.won {
            s.survivors += o.survivors;
            s.days += o.days;
        }
        for l in o.losses {
            *s.losses.entry(l).or_default() += 1;
        }
    }
    s
}

pub(crate) fn report(p: Persona, s: &Stats) {
    let w = s.wins.max(1);
    println!(
        "{:26} won {:3}/{:3} ({:3}%)  stuck {:2}  survivors {:.1}  days {:3}  losses {:?}",
        p.name,
        s.wins,
        s.trips,
        s.wins * 100 / s.trips.max(1),
        s.stuck,
        s.survivors as f32 / w as f32,
        s.days / w,
        s.losses
    );
}

#[test]
#[ignore = "only with PLAYTEST_SHOTS=dir: pictures of one kid's trip"]
fn kid_trip_pictures() {
    let o = play(KID, 7);
    println!("kid trip: {o:?}");
}

#[test]
#[ignore = "slow: run with --ignored --nocapture to see the difficulty table"]
fn playtest() {
    for p in [
        KID,
        KID_HELPED,
        KID_SEASONED,
        ADULT,
        KID_SURVIVAL,
        ADULT_SURVIVAL,
    ] {
        let s = run(p, 0..200);
        report(p, &s);
    }
}

/// Younger pioneers stay winnable for a young kid; seasoned is harder; survival is brutal
/// but a careful player can make it.
#[test]
fn difficulty_stays_in_line() {
    let rate = |p: Persona, n: u64| {
        let s = run(p, 1000..1000 + n);
        s.wins * 100 / s.trips
    };
    let kid = rate(KID, 30);
    let adult = rate(ADULT, 30);
    let kid_seasoned = rate(KID_SEASONED, 30);
    let survival = rate(ADULT_SURVIVAL, 40);
    println!("kid {kid}%  kid seasoned {kid_seasoned}%  adult {adult}%  survival {survival}%");
    assert!(
        kid >= 90,
        "younger pioneers too hard for a young kid: {kid}%"
    );
    assert!(
        adult >= 75,
        "seasoned too hard for a careful player: {adult}%"
    );
    assert!(
        kid_seasoned < 100 || adult < 100,
        "seasoned is not harder than younger"
    );
    assert!(
        (10..=75).contains(&survival),
        "survival out of line: {survival}%"
    );
}
