//! Races, just for fun, as a nod to the old DOS games: a wagon race (tap SPACE to
//! hurry the oxen, change lanes around rocks, bumps and trees), a foot race (jump
//! the logs and rocks), and a sled race down a snowy hill when there's snow about.
//!
//! Four racers line up. Alone, you race three computer racers: one fair, one easy
//! and one good (but not too good: this is a kids' game). Playing together (the
//! "Fun races" game), up to four players race, and computer racers fill the empty
//! spots. The host runs the race and shares its screen, like hunting together;
//! the screen follows the players, and anyone who falls too far behind is
//! pulled along to the edge of it, so nobody gets lost.
//!
//! Nothing on the trip changes, and the races have their own dice so the trip's
//! luck stays the same for everyone playing together.

use alloc::{boxed::Box, format, string::String, vec::Vec};

use embedded_graphics::prelude::RgbColor;

use crate::{
    Game, Held, Key, Precip, Scene,
    data::Terrain,
    gfx::{self, Color, Font, Frame, H, W},
    net, pictures,
    rng::Rng,
    sprites, travel, ui,
};

const FP: i32 = 16;
const TOP: i32 = 12;
const RACERS: usize = 4;
/// "3, 2, 1" takes this long, then it's "GO!".
const START: u32 = 2400;
/// "You win!" shows at least this long before ENTER goes on, so a player still
/// tapping away doesn't skip past it.
const CHEER_MS: u32 = 1000;
/// Once someone crosses the finish line, everyone else has this long to finish.
const END_WAIT: u32 = 6000;
/// Names for computer racers when there's nobody from the party to race.
const RIVALS: [&str; 8] = [
    "Jeb", "Clara", "Amos", "Hattie", "Eli", "Martha", "Silas", "Nell",
];
/// Racing alone you're blue, and the computer racers are these.
const COMPUTER_COLORS: [Color; 3] = [gfx::RED, gfx::rgb(240, 200, 0), gfx::PURPLE];
const ORDINALS: [&str; RACERS] = ["first", "second", "third", "fourth"];

// Wagon race. Oxen are slow, even when they hurry: 48 pixels a second is about 3 mph.
const LANES: i32 = 4;
const TEAM_X: i32 = 110;
const OX_TOP: i32 = 48;
const OX_CRAWL: i32 = 6;
const OX_URGE: i32 = 9;

// Foot race. Each runner has a track; these are the ground lines, top to bottom.
const RUN_X: i32 = 60;
const RUN: i32 = 64;
const RUN_ACCEL: i32 = 80;
const JUMP: i32 = 170;
const GRAVITY: i32 = 450;
const TRACKS: [i32; RACERS] = [64, 87, 110, 133];

// Sled race.
const SLED_Y: i32 = 30;
const STEER: i32 = 85;
/// Where each sled starts, across the hill.
const SLED_STARTS: [i32; RACERS] = [90, 140, 40, 190];

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Wagon,
    Foot,
    Sled,
}

impl Kind {
    fn name(self) -> &'static str {
        match self {
            Kind::Wagon => "Wagon race",
            Kind::Foot => "Foot race",
            Kind::Sled => "Sled race",
        }
    }

    /// How long the course is, in pixels.
    fn len(self) -> i32 {
        match self {
            Kind::Wagon => 2000,
            Kind::Foot => 1600,
            Kind::Sled => 3000,
        }
    }

    /// Playing together, the players can be at most this far apart (pixels), so
    /// everyone stays on the shared screen.
    fn spread(self) -> i32 {
        match self {
            Kind::Wagon => 170,
            Kind::Foot => 190,
            Kind::Sled => 90,
        }
    }

    /// Playing together, how far the middle of the pack is from the spot the
    /// screen is drawn around.
    fn middle(self) -> i32 {
        match self {
            Kind::Wagon => 10,
            Kind::Foot => 55,
            Kind::Sled => 37,
        }
    }
}

/// Where the games were picked from, to go back there.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum From {
    Title,
    Trail,
    /// The "Fun races" game when playing together.
    Together,
}

/// How good a computer racer is.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Skill {
    Easy,
    Fair,
    Good,
}

impl Skill {
    /// Wagon race: how long between hurrying the oxen (ms).
    fn hurry_ms(self) -> (i32, i32) {
        match self {
            Skill::Easy => (680, 940),
            Skill::Fair => (600, 840),
            Skill::Good => (520, 760),
        }
    }

    /// How often (out of 1000) they see trouble coming and get out of its way.
    /// They never miss twice in a row.
    fn dodge(self) -> u32 {
        match self {
            Skill::Easy => 820,
            Skill::Fair => 890,
            Skill::Good => 940,
        }
    }

    /// Foot race: running speed. You run at 64.
    fn run(self) -> i32 {
        match self {
            Skill::Easy => 57,
            Skill::Fair => 59,
            Skill::Good => 61,
        }
    }

    /// Sled race: top speed. Holding SPACE, you go 125.
    fn sled_top(self) -> i32 {
        match self {
            Skill::Easy => 96,
            Skill::Fair => 102,
            Skill::Good => 108,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Thing {
    Rock,
    Bump,
    Tree,
    Log,
    Pine,
}

struct Ob {
    /// How far along the course, in pixels.
    at: i32,
    /// Wagon race: the lane (0 is the top). Sled race: how far across, in pixels.
    side: i32,
    thing: Thing,
    /// Which racers have already run into it, one bit each.
    hit: u8,
}

struct Racer {
    name: String,
    color: Color,
    /// The player's id when playing together.
    id: u32,
    /// None for a person, or how good a computer racer is.
    ai: Option<Skill>,
    /// How far along, in 1/16 pixels.
    pos: i32,
    /// In 1/16 pixels a second.
    speed: i32,
    /// Wagon race: the lane. Sled race: how far across, in 1/16 pixels.
    side: i32,
    /// Wagon race: where the wagon is drawn, sliding toward its lane (1/16 pixels).
    y: i32,
    /// Foot race: height off the ground and how fast it's rising (1/16 pixels).
    up: i32,
    vy: i32,
    /// Tripped, spilled or stuck for this many more ms.
    hurt: u32,
    /// Shouting "Hyah!" / bouncing over a bump, for this many more ms.
    shout: u32,
    bounce: u32,
    /// The race clock when they crossed the finish line.
    done: Option<u32>,
    /// A computer racer: ms until it next hurries its oxen,
    think: u32,
    /// the last obstacle it thought about,
    seen: Option<usize>,
    /// and what it decided: when to jump (foot), to change lanes (wagon, 1) or
    /// where to steer (sled).
    plan: i32,
    /// Whether it missed the last one, so it won't miss the next.
    missed: bool,
}

impl Racer {
    /// A computer racer decides whether it'll see this one coming (`permille`
    /// times out of 1000).
    fn will_dodge(&mut self, permille: u32, rng: &mut Rng) -> bool {
        let dodge = self.missed || rng.chance(permille);
        self.missed = !dodge;
        dodge
    }
}

impl Racer {
    fn new(name: String, color: Color, id: u32, ai: Option<Skill>) -> Self {
        Racer {
            name,
            color,
            id,
            ai,
            pos: 0,
            speed: 0,
            side: 0,
            y: 0,
            up: 0,
            vy: 0,
            hurt: 0,
            shout: 0,
            bounce: 0,
            done: None,
            think: 0,
            seen: None,
            plan: 0,
            missed: false,
        }
    }

    fn person(&self) -> bool {
        self.ai.is_none()
    }
}

pub struct RaceState {
    kind: Kind,
    from: From,
    /// You (this device) first, then the other players, then the computer racers.
    r: Vec<Racer>,
    obs: Vec<Ob>,
    rng: Rng,
    /// Since the countdown started.
    clock: u32,
    beeped: u32,
    /// Counting down once the first racer finishes.
    end_in: Option<u32>,
    /// Time since the race ended, and who came in first, second...
    cheer: Option<u32>,
    places: Vec<usize>,
    terrain: Terrain,
    flash: Option<(String, u32)>,
}

impl RaceState {
    /// Players racing each other on their own devices.
    pub fn together(&self) -> bool {
        self.from == From::Together
    }
}

/// Is there snow about for sledding?
pub fn snowy(g: &Game) -> bool {
    let m = ui::month_index(g);
    g.t.precip == Precip::Snow
        || g.t.temp < 32
        || (travel::terrain(g) == Terrain::Mountains && !(4..=8).contains(&m))
}

fn games(g: &Game, from: From) -> Vec<Kind> {
    let mut v = alloc::vec![Kind::Wagon, Kind::Foot];
    if from != From::Trail || snowy(g) {
        v.push(Kind::Sled);
    }
    v
}

pub fn render_games(g: &Game, f: &mut Frame, from: From) {
    let together = from == From::Together;
    ui::header(
        f,
        if together {
            "Race together"
        } else {
            "Fun and games"
        },
    );
    let names: Vec<&str> = games(g, from).iter().map(|k| k.name()).collect();
    let mut y = ui::menu(f, 3, 18, &names) + 4;
    let about = if together {
        format!(
            "Up to {RACERS} racers. Computer racers fill the empty spots. {} picks the race.",
            g.net.name
        )
    } else {
        String::from("Race three computer racers, just for fun! Nothing on your trip changes.")
    };
    y = ui::para(f, 3, y, &about, gfx::GRAY);
    if from == From::Trail && !snowy(g) {
        ui::para(
            f,
            3,
            y,
            "Sledding needs snow. Look for it in the mountains!",
            gfx::GRAY,
        );
    }
    // Racers going by along the bottom.
    let x = W - ((g.clock / 50) as i32 % (W + 60));
    f.rect(0, 108, W, 14, gfx::rgb(90, 150, 60));
    pictures::team(f, x, 96, (g.clock / 250) as usize);
    let rx = (g.clock / 25) as i32 % (W + 40) - 20;
    f.sprite_accent(
        rx,
        99,
        &sprites::RUNNER[(g.clock / 150) as usize % 2],
        false,
        gfx::BLUE,
    );
    ui::footer(
        f,
        if together {
            "Pick a race, or ESC to finish."
        } else {
            "Pick a game, or ESC to go back."
        },
    );
}

pub fn games_key(g: &mut Game, from: From, k: Key) -> Scene {
    if k == Key::Escape {
        return match from {
            From::Title => Scene::Title,
            From::Trail => Scene::SizeUp,
            From::Together => {
                crate::party::end(g, "The races are over. Thanks for racing!");
                net::leave(g);
                Scene::Online(net::Screen::Menu)
            }
        };
    }
    let list = games(g, from);
    match ui::digit(k).filter(|&i| i <= list.len()) {
        Some(i) => {
            g.beep(587, 25);
            g.net.party.presses.clear();
            Scene::Race(Box::new(RaceState::new(g, list[i - 1], from)))
        }
        None => Scene::Games(from),
    }
}

impl RaceState {
    fn new(g: &Game, kind: Kind, from: From) -> Self {
        let mut rng = Rng::new(g.rng.state() ^ (u64::from(g.clock) << 20) ^ kind as u64);
        let mut r = Vec::new();
        if from == From::Together {
            for p in g.net.players.iter().take(RACERS) {
                r.push(Racer::new(p.name.clone(), g.net.color(p.id), p.id, None));
            }
        } else {
            r.push(Racer::new(String::from("You"), gfx::BLUE, 0, None));
        }
        // On the trail you race people from your party, if anyone's along (not in a
        // wagon race: they're riding in yours).
        let mut names: Vec<String> = if from == From::Trail && kind != Kind::Wagon {
            g.t.party
                .iter()
                .skip(1)
                .filter(|p| !p.gone)
                .map(|p| p.name.clone())
                .collect()
        } else {
            Vec::new()
        };
        let people = r.len();
        for (n, skill) in [Skill::Fair, Skill::Easy, Skill::Good]
            .into_iter()
            .take(RACERS - people)
            .enumerate()
        {
            let name = if names.is_empty() {
                let free: Vec<&str> = RIVALS
                    .into_iter()
                    .filter(|n| !r.iter().any(|x: &Racer| x.name == *n))
                    .collect();
                String::from(free[rng.below(free.len() as u32) as usize])
            } else {
                names.remove(rng.below(names.len() as u32) as usize)
            };
            let color = if from == From::Together {
                net::COLORS[(people + n) % net::COLORS.len()]
            } else {
                COMPUTER_COLORS[n]
            };
            r.push(Racer::new(name, color, 0, Some(skill)));
        }
        for (i, racer) in r.iter_mut().enumerate() {
            racer.side = match kind {
                // You start in the bottom lane or track.
                Kind::Wagon => LANES - 1 - i as i32,
                Kind::Sled => SLED_STARTS[i] * FP,
                Kind::Foot => 0,
            };
            racer.y = lane_y(racer.side) * FP;
            racer.plan = racer.side;
        }
        let mut obs = Vec::new();
        let len = kind.len();
        let mut at = 150;
        while at < len - 60 {
            let ob = |side, thing| Ob {
                at,
                side,
                thing,
                hit: 0,
            };
            match kind {
                Kind::Wagon => {
                    let lane = rng.range(0, LANES - 1);
                    let thing = [Thing::Rock, Thing::Bump, Thing::Tree][rng.below(3) as usize];
                    obs.push(ob(lane, thing));
                    if rng.chance(350) {
                        let other = (lane + rng.range(1, LANES - 1)) % LANES;
                        obs.push(ob(other, [Thing::Rock, Thing::Bump][rng.below(2) as usize]));
                    }
                    at += rng.range(70, 120);
                }
                Kind::Foot => {
                    obs.push(ob(0, [Thing::Log, Thing::Rock][rng.below(2) as usize]));
                    at += rng.range(90, 170);
                }
                Kind::Sled => {
                    let x = rng.range(12, W - 22);
                    let thing = if rng.chance(700) {
                        Thing::Pine
                    } else {
                        Thing::Rock
                    };
                    obs.push(ob(x, thing));
                    if rng.chance(350) {
                        let x2 = (x + rng.range(50, 150)) % (W - 34) + 12;
                        obs.push(ob(x2, Thing::Pine));
                    }
                    at += rng.range(26, 50);
                }
            }
        }
        RaceState {
            kind,
            from,
            r,
            obs,
            rng,
            clock: 0,
            beeped: 0,
            end_in: None,
            cheer: None,
            places: Vec::new(),
            terrain: if from == From::Trail {
                travel::terrain(g)
            } else {
                Terrain::Plains
            },
            flash: None,
        }
    }
}

/// The top of a wagon in a lane.
fn lane_y(lane: i32) -> i32 {
    50 + lane * 16
}

/// The lane a wagon is nearest to while it slides over.
fn lane_at(y: i32) -> i32 {
    ((y / FP - lane_y(0) + 8) / 16).clamp(0, LANES - 1)
}

/// Up or down keys: -1 or 1.
fn up_down(k: Key) -> i32 {
    match k {
        Key::Up | Key::Char('w' | 'W' | ';') => -1,
        Key::Down | Key::Char('s' | 'S' | '.') => 1,
        _ => 0,
    }
}

fn hurry(r: &mut Racer) {
    if r.hurt == 0 {
        r.speed = (r.speed + OX_URGE * FP).min(OX_TOP * FP);
    }
}

fn jump(r: &mut Racer) -> bool {
    if r.up == 0 && r.hurt == 0 {
        r.vy = JUMP * FP;
        r.up = 1;
        return true;
    }
    false
}

/// What a racer's key does (held keys are read in `tick`). Returns a sound to make.
fn act(r: &mut Racer, kind: Kind, k: Key) -> Option<&'static [(u16, u16)]> {
    let go = matches!(k, Key::Char(' ') | Key::Enter);
    match kind {
        Kind::Wagon => {
            let d = up_down(k);
            if d != 0 {
                r.side = (r.side + d).clamp(0, LANES - 1);
            } else if go && r.hurt == 0 {
                hurry(r);
                r.shout = 350;
                return Some(&[(140, 15)]);
            }
        }
        Kind::Foot => {
            if (go || up_down(k) < 0) && jump(r) {
                return Some(&[(523, 30), (784, 40)]);
            }
        }
        Kind::Sled => {}
    }
    None
}

pub fn key(g: &mut Game, mut s: Box<RaceState>, k: Key) -> Scene {
    if k == Key::Escape {
        return Scene::Games(s.from);
    }
    if let Some(t) = s.cheer {
        if t >= CHEER_MS && k == Key::Enter {
            return finish(g, &s);
        }
        return Scene::Race(s);
    }
    if s.clock >= START
        && s.r[0].done.is_none()
        && let Some(tune) = act(&mut s.r[0], s.kind, k)
    {
        g.tune(tune);
    }
    Scene::Race(s)
}

pub fn tick(g: &mut Game, mut s: Box<RaceState>, ms: u32) -> Scene {
    s.clock += ms;
    while s.beeped < 4 && s.clock >= s.beeped * START / 3 {
        g.beep(if s.beeped == 3 { 880 } else { 440 }, 150);
        s.beeped += 1;
    }
    let presses = core::mem::take(&mut g.net.party.presses);
    if s.clock < START {
        return Scene::Race(s);
    }
    if let Some(t) = &mut s.cheer {
        *t += ms;
    }
    // The other players' keys, when playing together. A player who left the game
    // hands their racer to the computer.
    let mut held = alloc::vec![Held::default(); s.r.len()];
    held[0] = g.held;
    if s.together() {
        for (i, r) in s.r.iter_mut().enumerate().skip(1) {
            if r.person() && !g.net.players.iter().any(|p| p.id == r.id) {
                r.ai = Some(Skill::Fair);
            }
            if r.person() {
                held[i] = g.net.party.held_of(r.id);
            }
        }
        for (id, k) in presses {
            let kind = s.kind;
            if let Some(r) =
                s.r.iter_mut()
                    .skip(1)
                    .find(|r| r.person() && r.id == id && r.done.is_none())
                && s.cheer.is_none()
                && let Some(tune) = act(r, kind, k)
            {
                g.tune(tune);
            }
        }
    }
    let dt = ms as i32;
    for r in &mut s.r {
        r.hurt = r.hurt.saturating_sub(ms);
        r.shout = r.shout.saturating_sub(ms);
        r.bounce = r.bounce.saturating_sub(ms);
    }
    let hits = match s.kind {
        Kind::Wagon => wagon_tick(&mut s, dt),
        Kind::Foot => foot_tick(&mut s, dt),
        Kind::Sled => sled_tick(&mut s, &held, dt),
    };
    for (i, what) in hits {
        if i == 0 && !s.together() {
            s.flash = Some((String::from(what), 1200));
        }
        if s.r[i].person() {
            g.tune(&[(150, 60), (100, 120)]);
        }
    }
    if s.together() {
        keep_together(&mut s);
    }
    let now = s.clock - START;
    let len = s.kind.len();
    for i in 0..s.r.len() {
        if s.r[i].done.is_none() && s.r[i].pos / FP >= len {
            s.r[i].done = Some(now);
            s.end_in.get_or_insert(END_WAIT);
            if s.r[i].person() && s.cheer.is_none() {
                g.beep(988, 80);
            }
        }
    }
    if let Some(t) = &mut s.end_in {
        *t = t.saturating_sub(ms);
    }
    let people_done = s.r.iter().filter(|r| r.person()).all(|r| r.done.is_some());
    if s.cheer.is_none() && (people_done || s.end_in == Some(0)) {
        s.places = standings(&s);
        s.cheer = Some(0);
        if s.together() || s.places[0] == 0 {
            g.tune(&[(523, 100), (659, 100), (784, 100), (1047, 260)]);
        } else {
            g.tune(&[(392, 150), (330, 150), (392, 260)]);
        }
    }
    if let Some((_, t)) = &mut s.flash {
        *t = t.saturating_sub(ms);
        if *t == 0 {
            s.flash = None;
        }
    }
    Scene::Race(s)
}

/// Who came in first, second...: finishers by time, then the rest by how far they got.
fn standings(s: &RaceState) -> Vec<usize> {
    let mut v: Vec<usize> = (0..s.r.len()).collect();
    v.sort_by_key(|&i| match s.r[i].done {
        Some(t) => (0, i64::from(t)),
        None => (1, -i64::from(s.r[i].pos)),
    });
    v
}

/// Playing together, nobody falls off the shared screen: anyone too far behind
/// the leading player is pulled along to the edge of it.
fn keep_together(s: &mut RaceState) {
    let Some(lead) = s.r.iter().filter(|r| r.person()).map(|r| r.pos).max() else {
        return;
    };
    let back = lead - s.kind.spread() * FP;
    for r in s.r.iter_mut().filter(|r| r.person()) {
        r.pos = r.pos.max(back);
    }
}

/// The spot on the course the screen is drawn around: you, or playing together,
/// the middle of the players.
fn camera(s: &RaceState) -> i32 {
    if !s.together() {
        return s.r[0].pos / FP;
    }
    let people = s.r.iter().filter(|r| r.person()).map(|r| r.pos / FP);
    let (lo, hi) = people.fold((i32::MAX, i32::MIN), |(lo, hi), p| (lo.min(p), hi.max(p)));
    if lo > hi {
        return s.r[0].pos / FP;
    }
    (lo + hi) / 2 - s.kind.middle()
}

/// A computer driver hurries the oxen now and then, and steers around trouble
/// into a lane that's clear for a while.
fn ai_wagon(r: &mut Racer, skill: Skill, obs: &[Ob], rng: &mut Rng, dt: i32) {
    r.think = r.think.saturating_sub(dt as u32);
    if r.think == 0 {
        hurry(r);
        let (lo, hi) = skill.hurry_ms();
        r.think = rng.range(lo, hi) as u32;
    }
    let (front, lane) = (r.pos / FP, r.side);
    let Some(i) = obs
        .iter()
        .position(|o| o.side == lane && o.at > front - 2 && o.at - front < 75)
    else {
        return;
    };
    if r.seen != Some(i) {
        r.seen = Some(i);
        r.plan = i32::from(r.will_dodge(skill.dodge(), rng));
    }
    if r.plan == 0 {
        return;
    }
    // A lane is clear if nothing's in it from beside the wagon to a little past
    // the thing in the way. Until one is, keep looking.
    let past = obs[i].at + 50;
    let clear = |l: i32| {
        (0..LANES).contains(&l)
            && !obs
                .iter()
                .any(|o| o.side == l && o.at > front - 40 && o.at < past)
    };
    let ways: Vec<i32> = [lane - 1, lane + 1]
        .into_iter()
        .filter(|&l| clear(l))
        .collect();
    if !ways.is_empty() {
        r.side = ways[rng.below(ways.len() as u32) as usize];
        r.plan = 0;
    }
}

/// A computer runner jumps each log or rock, usually in time.
fn ai_foot(r: &mut Racer, skill: Skill, obs: &[Ob], rng: &mut Rng) {
    let x = r.pos / FP;
    let Some(i) = obs.iter().position(|o| o.at + 4 > x) else {
        return;
    };
    if r.seen != Some(i) {
        r.seen = Some(i);
        // The jump clears a log when it starts 9 to 31 pixels before it.
        r.plan = if r.will_dodge(skill.dodge(), rng) {
            rng.range(12, 26)
        } else {
            0
        };
    }
    if r.plan > 0 && obs[i].at - x <= r.plan {
        jump(r);
    }
}

/// A computer sledder steers round trees and rocks, on whichever side is clear.
/// Returns which way it's steering.
fn ai_sled(r: &mut Racer, skill: Skill, obs: &[Ob], rng: &mut Rng) -> i32 {
    let (x, y) = (r.side / FP, r.pos / FP);
    if let Some(i) = obs
        .iter()
        .position(|o| o.at > y + 4 && o.at - y < 85 && (o.side - x).abs() < 14)
        && r.seen != Some(i)
    {
        r.seen = Some(i);
        // With trees all over the hill, it misses half as often.
        if r.will_dodge(1000 - (1000 - skill.dodge()) / 2, rng) {
            let o = &obs[i];
            let free = |tx: i32| {
                (12..=W - 22).contains(&tx)
                    && !obs
                        .iter()
                        .any(|p| (p.side - tx).abs() < 13 && p.at > y + 4 && p.at < o.at + 40)
            };
            let (left, right) = (o.side - 15, o.side + 15);
            let order = if x <= o.side {
                [left, right]
            } else {
                [right, left]
            };
            if let Some(tx) = order.into_iter().find(|&tx| free(tx)) {
                r.plan = tx * FP;
            }
        }
    }
    let d = r.plan - r.side;
    if d.abs() > 2 * FP { d.signum() } else { 0 }
}

/// Moves the wagons. Returns who hit what.
fn wagon_tick(s: &mut RaceState, dt: i32) -> Vec<(usize, &'static str)> {
    let RaceState { r, obs, rng, .. } = s;
    let mut hits = Vec::new();
    for (i, r) in r.iter_mut().enumerate() {
        if let Some(skill) = r.ai {
            ai_wagon(r, skill, obs, rng, dt);
        }
        let step = 90 * FP * dt / 1000;
        r.y += (lane_y(r.side) * FP - r.y).clamp(-step, step);
        if r.hurt > 0 {
            r.speed = 0;
        } else {
            // The oxen slow down unless someone keeps after them.
            r.speed = (r.speed - r.speed * dt * 2 / 5000).max(OX_CRAWL * FP);
        }
        r.pos += r.speed * dt / 1000;
        let (front, lane) = (r.pos / FP, lane_at(r.y));
        for ob in obs.iter_mut() {
            let w = match ob.thing {
                Thing::Bump => 12,
                _ => 10,
            };
            if ob.hit & (1 << i) != 0
                || ob.side != lane
                || ob.at > front + w - 2
                || ob.at < front - 36
            {
                continue;
            }
            ob.hit |= 1 << i;
            let what = match ob.thing {
                Thing::Bump => {
                    r.speed /= 2;
                    r.bounce = 500;
                    "Bump!"
                }
                Thing::Tree => {
                    r.hurt = 1500;
                    "Whoa! A tree!"
                }
                _ => {
                    r.hurt = 1000;
                    "Whoa! A rock!"
                }
            };
            hits.push((i, what));
        }
    }
    hits
}

fn foot_tick(s: &mut RaceState, dt: i32) -> Vec<(usize, &'static str)> {
    let RaceState { r, obs, rng, .. } = s;
    let mut hits = Vec::new();
    for (i, r) in r.iter_mut().enumerate() {
        if let Some(skill) = r.ai {
            ai_foot(r, skill, obs, rng);
        }
        let run = r.ai.map_or(RUN, Skill::run);
        r.speed = if r.hurt > 0 {
            12 * FP
        } else {
            (r.speed + RUN_ACCEL * FP * dt / 1000).min(run * FP)
        };
        r.pos += r.speed * dt / 1000;
        if r.up > 0 {
            r.up += r.vy * dt / 1000;
            r.vy -= GRAVITY * FP * dt / 1000;
            if r.up <= 0 {
                r.up = 0;
                r.vy = 0;
            }
        }
        let x = r.pos / FP;
        for ob in obs.iter_mut() {
            let (w, h) = if ob.thing == Thing::Log {
                (16, 7)
            } else {
                (10, 6)
            };
            if ob.hit & (1 << i) != 0 || ob.at >= x + 6 || ob.at + w <= x + 2 || r.up / FP >= h {
                continue;
            }
            ob.hit |= 1 << i;
            r.hurt = 900;
            hits.push((
                i,
                if ob.thing == Thing::Log {
                    "Oops! Tripped on a log."
                } else {
                    "Oops! Tripped on a rock."
                },
            ));
        }
    }
    hits
}

fn sled_tick(s: &mut RaceState, held: &[Held], dt: i32) -> Vec<(usize, &'static str)> {
    let RaceState { r, obs, rng, .. } = s;
    let mut hits = Vec::new();
    for (i, r) in r.iter_mut().enumerate() {
        let (steer, (top, accel)) = match r.ai {
            Some(skill) => (ai_sled(r, skill, obs, rng), (skill.sled_top(), 45)),
            None => {
                let h = held[i];
                let fast = h.down || h.fire;
                (
                    i32::from(h.right) - i32::from(h.left),
                    if fast { (125, 55) } else { (90, 35) },
                )
            }
        };
        if r.hurt > 0 {
            r.speed = 0;
        } else {
            let step = accel * FP * dt / 1000;
            r.speed = if r.speed < top * FP {
                (r.speed + step).min(top * FP)
            } else {
                (r.speed - step).max(top * FP)
            };
            r.side = (r.side + steer * STEER * FP * dt / 1000).clamp(8 * FP, (W - 18) * FP);
        }
        r.pos += r.speed * dt / 1000;
        let (x, y) = (r.side / FP, r.pos / FP);
        for ob in obs.iter_mut() {
            // Only the bottom of a tree, and the rock itself, are in the way.
            let (bx, bw, by, bh) = match ob.thing {
                Thing::Pine => (1, 7, 5, 6),
                _ => (0, 10, 1, 5),
            };
            let (ox, oy) = (ob.side + bx, ob.at + by);
            if ob.hit & (1 << i) != 0
                || x + 9 <= ox
                || x + 1 >= ox + bw
                || y + 10 <= oy
                || y + 3 >= oy + bh
            {
                continue;
            }
            ob.hit |= 1 << i;
            r.hurt = 1100;
            hits.push((
                i,
                if ob.thing == Thing::Pine {
                    "Crash! Into a tree!"
                } else {
                    "Crash! Into a rock!"
                },
            ));
        }
    }
    hits
}

fn finish(g: &mut Game, s: &RaceState) -> Scene {
    if s.together() {
        // Everyone saw the results on the shared screen.
        return Scene::Games(s.from);
    }
    let what = s.kind.name().to_ascii_lowercase();
    let place = s.places.iter().position(|&i| i == 0).unwrap_or(0);
    let name = |n: usize| &s.r[s.places[n]].name;
    let body = if place == 0 {
        format!("You won the {what}! {} came in second.", name(1))
    } else {
        format!(
            "{} won the {what}, and you came in {}. Want a rematch?",
            name(0),
            ORDINALS[place]
        )
    };
    let fact = match s.kind {
        Kind::Wagon => {
            "Real oxen walked about 2 miles an hour, no faster than you walk. A good day on the trail was about 15 miles."
        }
        Kind::Foot => {
            "Pioneer children walked most of the way to Oregon, about 2,000 miles! The wagons were full of supplies."
        }
        Kind::Sled => {
            "Wagons had to cross the mountains before the snow came. Snow could close the passes for the whole winter."
        }
    };
    let title = if place == 0 { "You won!" } else { "Good race!" };
    g.say(title, format!("{body}\n\nDid you know? {fact}"));
    Scene::Games(s.from)
}

// ---------------------------------------------------------------- drawing

/// Black or white, whichever reads better on this color.
fn ink(c: Color) -> Color {
    let light =
        u32::from(c.r()) * 8 * 299 + u32::from(c.g()) * 4 * 587 + u32::from(c.b()) * 8 * 114;
    if light > 150_000 {
        gfx::BLACK
    } else {
        gfx::WHITE
    }
}

fn name_tag(f: &mut Frame, x: i32, y: i32, r: &Racer) {
    f.label(x, y, &r.name, ink(r.color), r.color, Font::Small);
}

pub fn render(g: &Game, f: &mut Frame, s: &RaceState) {
    match s.kind {
        Kind::Wagon => render_wagon(g, f, s),
        Kind::Foot => render_foot(g, f, s),
        Kind::Sled => render_sled(g, f, s),
    }
    hud(f, s);
    if s.clock < START + 700 {
        let n = s.clock * 3 / START;
        let word = ["3", "2", "1", "GO!"][n.min(3) as usize];
        let w = word.len() as i32 * 10;
        f.label((W - w) / 2, 22, word, gfx::YELLOW, gfx::BLACK, Font::Big);
    }
    if s.clock < START + 2500 && s.cheer.is_none() {
        let hint = match s.kind {
            Kind::Wagon => "SPACE: hurry the oxen!  UP/DOWN: lane",
            Kind::Foot => "SPACE: jump over logs and rocks!",
            Kind::Sled => "LEFT/RIGHT: steer  hold SPACE: faster",
        };
        let w = hint.len() as i32 * 5;
        f.label(
            (W - w) / 2,
            H - 10,
            hint,
            gfx::WHITE,
            gfx::BLACK,
            Font::Small,
        );
    }
    if let Some((msg, _)) = &s.flash {
        let w = msg.len() as i32 * 6;
        f.label((W - w) / 2, 15, msg, gfx::YELLOW, gfx::BLACK, Font::Normal);
    }
    if let Some(t) = s.cheer {
        render_cheer(f, s, t);
    }
}

fn render_cheer(f: &mut Frame, s: &RaceState, t: u32) {
    let first = &s.r[s.places[0]];
    let (word, c) = if s.together() {
        (format!("{} wins!", first.name), first.color)
    } else {
        let place = s.places.iter().position(|&i| i == 0).unwrap_or(0);
        let word = ["You win!", "2nd place!", "3rd place!", "4th place"][place];
        (String::from(word), gfx::YELLOW)
    };
    let w = word.len() as i32 * 10;
    f.label((W - w) / 2, 18, &word, c, gfx::BLACK, Font::Big);
    if t < CHEER_MS {
        return;
    }
    if s.together() {
        // Everyone's place, on the shared screen ("1st", not "1.", which would
        // look like a menu choice).
        f.rect(60, 42, 120, 12 + s.places.len() as i32 * 11, gfx::BLACK);
        f.outline(60, 42, 120, 12 + s.places.len() as i32 * 11, gfx::GRAY);
        for (n, &i) in s.places.iter().enumerate() {
            let r = &s.r[i];
            let y = 48 + n as i32 * 11;
            f.rect(66, y + 1, 6, 6, r.color);
            f.text(
                76,
                y,
                &format!("{} {}", ["1st", "2nd", "3rd", "4th"][n], r.name),
                gfx::WHITE,
                Font::Normal,
            );
        }
    }
    let go_on = if s.together() {
        "Host: press ENTER"
    } else {
        "Press ENTER"
    };
    let w = go_on.len() as i32 * 6;
    f.label(
        (W - w) / 2,
        H - 11,
        go_on,
        gfx::WHITE,
        gfx::BLACK,
        Font::Normal,
    );
}

/// The top bar: a little track from start to finish with every racer on it.
fn hud(f: &mut Frame, s: &RaceState) {
    f.rect(0, 0, W, TOP, gfx::BLACK);
    let left = if s.kind == Kind::Wagon && !s.together() {
        // Pixels a second to miles an hour, rounded: 48 is 3 mph.
        format!("{} mph", (s.r[0].speed / FP + 8) / 16)
    } else {
        let t = s.r[0].done.unwrap_or(s.clock.saturating_sub(START));
        format!("{}s", t / 1000)
    };
    f.text(3, 2, &left, gfx::WHITE, Font::Small);
    let (x0, x1, len) = (52, 226, s.kind.len());
    f.rect(x0, 6, x1 - x0, 1, gfx::GRAY);
    checkers(f, x1, 2, 6, 8);
    // People on top, and you on top of everyone.
    let mut order: Vec<usize> = (0..s.r.len()).rev().collect();
    order.sort_by_key(|&i| (s.r[i].person(), i == 0));
    for i in order {
        let r = &s.r[i];
        let x = x0 + (x1 - x0) * (r.pos / FP).min(len) / len;
        f.rect(x - 2, 3, 5, 7, r.color);
    }
}

/// A black and white checkered block, like a finish flag.
fn checkers(f: &mut Frame, x: i32, y: i32, w: i32, h: i32) {
    for dy in 0..h {
        for dx in 0..w {
            let c = if (dx / 2 + dy / 2) % 2 == 0 {
                gfx::WHITE
            } else {
                gfx::BLACK
            };
            f.pixel(x + dx, y + dy, c);
        }
    }
}

/// When a racer (`size` pixels long) is off the screen, show which way they are.
fn edge_marker(f: &mut Frame, pos: i32, across: i32, sideways: bool, size: i32, c: Color) {
    let (lo, hi) = if sideways {
        (-size, W)
    } else {
        (TOP - size, H)
    };
    if pos > lo && pos < hi {
        return;
    }
    let near = if pos <= lo { lo.max(0) + 2 } else { hi - 6 };
    let (x, y) = if sideways {
        (near.clamp(2, W - 6), across)
    } else {
        (across, near.clamp(TOP + 2, H - 6))
    };
    f.rect(x, y, 5, 5, c);
    f.outline(x - 1, y - 1, 7, 7, gfx::WHITE);
}

fn render_wagon(g: &Game, f: &mut Frame, s: &RaceState) {
    let cam = camera(s);
    pictures::backdrop(f, s.terrain, TOP, 44, cam, gfx::SKY);
    f.rect(0, 46, W, 70, gfx::rgb(170, 130, 80));
    f.rect(0, 116, W, H - 116, pictures::ground_color(s.terrain));
    for l in 1..LANES {
        let y = lane_y(l) - 2;
        for k in 0..12 {
            let x = (k * 24 + cam).rem_euclid(W + 24) - 24;
            f.rect(x, y, 10, 1, gfx::rgb(200, 165, 110));
        }
    }
    // Things ahead (farther along) are to the left: the trail goes west.
    let sx = |at: i32| TEAM_X - (at - cam);
    let finish = sx(s.kind.len());
    if finish > -10 && finish < W {
        checkers(f, finish, 46, 4, 70);
    }
    for lane in 0..LANES {
        let base = lane_y(lane) + 14;
        for ob in s.obs.iter().filter(|o| o.side == lane) {
            let x = sx(ob.at);
            if !(-16..W + 4).contains(&x) {
                continue;
            }
            match ob.thing {
                Thing::Bump => {
                    pictures::dome(f, x + 6, base, 7, 4, gfx::DARK_BROWN);
                    pictures::dome(f, x + 6, base, 4, 2, gfx::BROWN);
                }
                Thing::Tree => f.sprite(x, base - 13, &sprites::TREE, false),
                _ => f.sprite(x, base - 6, &sprites::ROCK, false),
            }
        }
        for r in s.r.iter().rev().filter(|r| lane_at(r.y) == lane) {
            let mut x = sx(r.pos / FP);
            let mut y = r.y / FP;
            if r.bounce > 0 {
                y -= ((r.bounce / 100) % 2) as i32 * 2;
            }
            if r.hurt > 0 {
                x += ((g.clock / 60) % 2) as i32;
            }
            pictures::team(f, x, y, (r.pos / FP / 6) as usize);
            f.rect(x + 30, y - 6, 1, 6, gfx::DARK_BROWN);
            f.rect(x + 31, y - 6, 6, 4, r.color);
            name_tag(f, x + 42, y + 4, r);
            if r.shout > 0 && r.person() {
                f.label(x - 2, y - 2, "Hyah!", gfx::YELLOW, gfx::BLACK, Font::Small);
            }
        }
    }
    for r in &s.r {
        edge_marker(
            f,
            sx(r.pos / FP),
            lane_y(lane_at(r.y)) + 4,
            true,
            40,
            r.color,
        );
    }
}

fn render_foot(g: &Game, f: &mut Frame, s: &RaceState) {
    let cam = camera(s);
    pictures::backdrop(f, s.terrain, TOP, 40, -cam, gfx::SKY);
    f.rect(0, 44, W, H - 44, pictures::ground_color(s.terrain));
    let sx = |at: i32| RUN_X + (at - cam);
    let n = s.r.len();
    // Racer i runs on the i-th track from the bottom.
    let track = |i: usize| TRACKS[RACERS - 1 - i];
    for i in (0..n).rev() {
        let ground = track(i);
        f.rect(0, ground, W, 3, gfx::rgb(170, 130, 80));
        for ob in &s.obs {
            let x = sx(ob.at);
            if !(-20..W + 4).contains(&x) {
                continue;
            }
            if ob.thing == Thing::Log {
                f.rect(x, ground - 7, 16, 7, gfx::BROWN);
                f.rect(x, ground - 7, 16, 1, gfx::DARK_BROWN);
                f.rect(x, ground - 1, 16, 1, gfx::DARK_BROWN);
                f.rect(x + 12, ground - 7, 4, 7, gfx::TAN);
                f.rect(x + 13, ground - 5, 2, 3, gfx::BROWN);
            } else {
                f.sprite(x, ground - 6, &sprites::ROCK, false);
            }
        }
        let fin = sx(s.kind.len());
        if fin > -6 && fin < W {
            checkers(f, fin, ground - 16, 4, 16);
        }
        let r = &s.r[i];
        let x = sx(r.pos / FP);
        // Jumps are drawn half as high, so a runner stays over their own track.
        let y = ground - 11 - r.up / FP / 2;
        let frame = if r.up > 0 || r.hurt > 0 {
            2
        } else {
            (r.pos / FP / 7) as usize % 2
        };
        let wobble = if r.hurt > 0 {
            ((g.clock / 80) % 2) as i32
        } else {
            0
        };
        f.sprite_accent(x, y + wobble, &sprites::RUNNER[frame], false, r.color);
        let w = r.name.len() as i32 * 5;
        name_tag(f, x - w - 2, ground - 9, r);
        edge_marker(f, x, ground - 9, true, 8, r.color);
    }
}

fn render_sled(g: &Game, f: &mut Frame, s: &RaceState) {
    let cam = camera(s);
    f.rect(0, TOP, W, H - TOP, gfx::rgb(235, 240, 250));
    for i in 0..45 {
        let y = (i * 29 - cam).rem_euclid(H - TOP) + TOP;
        let x = (i * 53 + i * i) % W;
        f.rect(x, y, 3, 1, gfx::rgb(195, 205, 230));
    }
    // Trees along both sides of the run.
    for i in 0..8 {
        let y = (i * 26 - cam).rem_euclid(H - TOP + 12) + TOP - 12;
        f.sprite(-3 + (i % 2) * 2, y, &sprites::PINE, false);
        f.sprite(W - 7 - (i % 2) * 2, y + 13, &sprites::PINE, false);
    }
    // Things farther down the hill are lower on the screen.
    let sy = |at: i32| SLED_Y + (at - cam);
    let fin = sy(s.kind.len());
    if fin > TOP - 6 && fin < H {
        checkers(f, 0, fin, W, 6);
    }
    for ob in &s.obs {
        let y = sy(ob.at);
        if !(TOP - 12..H).contains(&y) {
            continue;
        }
        if ob.thing == Thing::Pine {
            f.sprite(ob.side, y, &sprites::PINE, false);
        } else {
            f.sprite(ob.side, y, &sprites::ROCK, false);
            f.rect(ob.side + 3, y, 4, 1, gfx::WHITE);
            f.rect(ob.side + 2, y + 1, 6, 1, gfx::WHITE);
        }
    }
    for r in s.r.iter().rev() {
        let (x, y) = (r.side / FP, sy(r.pos / FP));
        if r.hurt > 0 {
            // Tumbled into the snow.
            for k in 0..6 {
                let a = (g.clock / 70) as i32 + k * 3;
                f.rect(x + (a * 7) % 14 - 2, y + (a * 5) % 10, 2, 2, gfx::WHITE);
            }
        }
        if r.hurt == 0 || (g.clock / 120) % 2 == 0 {
            f.sprite_accent(x, y, &sprites::SLEDDER, false, r.color);
        }
        if r.speed > 100 * FP && r.hurt == 0 {
            for k in 0..3 {
                f.rect(x + 2 + k * 3, y - 6, 1, 4, gfx::rgb(170, 185, 220));
            }
        }
        let w = r.name.len() as i32 * 5;
        name_tag(f, x + 5 - w / 2, y - 10, r);
        edge_marker(f, y, x + 2, false, 10, r.color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays one race from the title screen. `good` plays carefully; otherwise
    /// nobody touches the keys. Returns your place (0 is first).
    fn race(seed: u64, pick: char, good: bool) -> usize {
        let mut g = Game::new(seed);
        g.key_down(Key::Char('6'));
        g.key_down(Key::Char(pick));
        assert!(matches!(g.scene, Scene::Race(_)));
        for step in 0..40_000u32 {
            let mut press = None;
            if let Scene::Race(s) = &g.scene {
                if s.cheer >= Some(CHEER_MS) {
                    return s.places.iter().position(|&i| i == 0).unwrap();
                }
                if good {
                    press = good_key(&mut g.held, s, step);
                }
            }
            if let Some(k) = press {
                g.key_down(k);
                g.key_up(k);
            }
            g.tick(16);
        }
        panic!("the race never ended");
    }

    /// What a careful player presses now.
    fn good_key(held: &mut Held, s: &RaceState, step: u32) -> Option<Key> {
        let you = &s.r[0];
        let (x, pos) = (you.side / FP, you.pos / FP);
        match s.kind {
            Kind::Wagon => {
                let lane = lane_at(you.y);
                let blocked = |l: i32| {
                    s.obs
                        .iter()
                        .any(|o| o.side == l && o.at > pos - 38 && o.at - pos < 45)
                };
                let to = (0..LANES).find(|&l| (l - lane).abs() == 1 && !blocked(l));
                if step % 20 == 0 {
                    Some(Key::Char(' '))
                } else if blocked(lane) && you.side == lane {
                    to.map(|l| if l < lane { Key::Up } else { Key::Down })
                } else {
                    None
                }
            }
            Kind::Foot => s
                .obs
                .iter()
                .any(|o| o.at - pos > 9 && o.at - pos <= 14)
                .then_some(Key::Char(' ')),
            Kind::Sled => {
                held.fire = true;
                let ahead = s
                    .obs
                    .iter()
                    .find(|o| o.at > pos && o.at - pos < 45 && (o.side - x).abs() < 14);
                held.left = ahead.is_some_and(|o| o.side >= x && x > 20);
                held.right = ahead.is_some_and(|o| !(o.side >= x && x > 20));
                None
            }
        }
    }

    #[test]
    fn careful_racers_win_and_idle_ones_lose() {
        for pick in ['1', '2', '3'] {
            let wins = (1..=6).filter(|&seed| race(seed, pick, true) == 0).count();
            assert!(
                wins >= 5,
                "game {pick}: a careful player won only {wins} of 6"
            );
            for seed in 1..=3 {
                assert_eq!(race(seed, pick, false), 3, "game {pick}: sitting still");
            }
        }
    }

    /// Computer racers left alone: how many obstacles each one hit.
    fn computer_hits(kind: Kind, seed: u64) -> Vec<(Skill, u32)> {
        let g = Game::new(seed);
        let mut s = RaceState::new(&g, kind, From::Title);
        s.clock = START;
        let held = alloc::vec![Held::default(); RACERS];
        for _ in 0..6000 {
            match kind {
                Kind::Wagon => wagon_tick(&mut s, 16),
                Kind::Foot => foot_tick(&mut s, 16),
                Kind::Sled => sled_tick(&mut s, &held, 16),
            };
            for r in &mut s.r {
                r.hurt = r.hurt.saturating_sub(16);
            }
        }
        (1..RACERS)
            .map(|i| {
                let n = s.obs.iter().filter(|o| o.hit & (1 << i) != 0).count();
                (s.r[i].ai.unwrap(), n as u32)
            })
            .collect()
    }

    #[test]
    fn computer_racers_mostly_dodge_and_jump() {
        for kind in [Kind::Wagon, Kind::Foot, Kind::Sled] {
            let total = |want: Skill| -> u32 {
                let mut hits = 0;
                for seed in 1..=8 {
                    for (skill, n) in computer_hits(kind, seed) {
                        if skill == want {
                            hits += n;
                        }
                    }
                }
                hits
            };
            // Eight races each: a good racer hits about one thing a race at most.
            let (good, fair, easy) = (total(Skill::Good), total(Skill::Fair), total(Skill::Easy));
            let name = kind.name();
            assert!(good <= 10, "{name}: a good racer hit {good} things");
            assert!(fair <= 16, "{name}: a fair racer hit {fair} things");
            assert!(easy <= 24, "{name}: an easy racer hit {easy} things");
        }
    }

    #[test]
    fn players_race_together_and_stay_on_screen() {
        let mut g = Game::new(5);
        g.net.racing = true;
        g.net.host = Some(g.net.me);
        g.net.settings.mode = net::Mode::Races;
        g.net.name = String::from("Ann");
        let mut ann = g.net.me_peer();
        ann.name = String::from("Ann");
        let mut bob = ann.clone();
        bob.id = 77;
        bob.name = String::from("Bob");
        g.net.players = alloc::vec![ann, bob];
        g.scene = Scene::Games(From::Together);
        g.key_down(Key::Char('1'));
        let Scene::Race(s) = &g.scene else {
            panic!("no race")
        };
        assert_eq!(s.r.len(), RACERS);
        assert_eq!(s.r.iter().filter(|r| r.person()).count(), 2);
        // Bob taps SPACE from his own device; Ann does nothing.
        for step in 0..20_000u32 {
            if step % 15 == 0 {
                crate::party::remote_key(&mut g, 77, true, Key::Char(' '));
                crate::party::remote_key(&mut g, 77, false, Key::Char(' '));
            }
            g.tick(16);
            let Scene::Race(s) = &g.scene else {
                panic!("the race went away")
            };
            let (a, b) = (s.r[0].pos / FP, s.r[1].pos / FP);
            assert!(b - a <= Kind::Wagon.spread(), "Ann fell off the screen");
            if s.cheer >= Some(CHEER_MS) {
                assert_eq!(s.r[s.places[0]].name, "Bob", "Bob tapped and won");
                g.key_down(Key::Enter);
                assert!(matches!(g.scene, Scene::Games(From::Together)));
                return;
            }
        }
        panic!("the race never ended");
    }

    #[test]
    fn sledding_needs_snow_on_the_trail() {
        let mut g = Game::new(3);
        g.t.precip = Precip::Clear;
        g.t.temp = 70;
        g.t.day = 160;
        assert!(!snowy(&g) || travel::terrain(&g) == Terrain::Mountains);
        g.t.precip = Precip::Snow;
        assert!(snowy(&g));
        assert_eq!(games(&g, From::Trail).len(), 3);
    }
}
