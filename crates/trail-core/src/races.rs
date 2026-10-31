//! Races, just for fun, as a nod to the old DOS games: a wagon race (tap SPACE to
//! hurry the oxen, change lanes around rocks, bumps and trees), a foot race (jump
//! the logs and rocks), and a sled race down a snowy hill when there's snow about.
//! Nothing on the trip changes, and the races have their own dice so the trip's
//! luck stays the same for everyone playing together.

use alloc::{boxed::Box, format, string::String, vec::Vec};

use crate::{
    Game, Key, Precip, Scene,
    data::Terrain,
    gfx::{self, Color, Font, Frame, H, W},
    pictures,
    rng::Rng,
    sprites, travel, ui,
};

const FP: i32 = 16;
const TOP: i32 = 12;
/// "3, 2, 1" takes this long, then it's "GO!".
const START: u32 = 2400;
/// "You win!" (or "Jeb wins!") shows at least this long before ENTER goes on, so a
/// player still tapping away doesn't skip past it.
const CHEER_MS: u32 = 1000;
/// Names for the other racer when there's nobody from the party to race.
const RIVALS: [&str; 8] = [
    "Jeb", "Clara", "Amos", "Hattie", "Eli", "Martha", "Silas", "Nell",
];
const YOU_COLOR: Color = gfx::BLUE;
const RIVAL_COLOR: Color = gfx::RED;

// Wagon race. Oxen are slow, even when they hurry: 48 pixels a second is about 3 mph.
const LANES: i32 = 3;
const TEAM_X: i32 = 110;
const OX_TOP: i32 = 48;
const OX_CRAWL: i32 = 6;
const OX_URGE: i32 = 9;

// Foot race.
const RUN_X: i32 = 60;
const RUN: i32 = 64;
const RIVAL_RUN: i32 = 60;
const RUN_ACCEL: i32 = 80;
const JUMP: i32 = 170;
const GRAVITY: i32 = 450;
const YOUR_TRACK: i32 = 118;
const RIVAL_TRACK: i32 = 80;

// Sled race.
const SLED_Y: i32 = 30;
const STEER: i32 = 85;

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
}

/// Where the games were picked from, to go back there.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum From {
    Title,
    Trail,
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
    /// Whether you (0) and the other racer (1) have already run into it.
    hit: [bool; 2],
}

#[derive(Default)]
struct Racer {
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
    /// The computer racer: ms until it next hurries its oxen,
    think: u32,
    /// the last obstacle it thought about,
    seen: Option<usize>,
    /// and what it decided: when to jump (foot) or where to steer (sled).
    plan: i32,
}

pub struct RaceState {
    kind: Kind,
    from: From,
    rival: String,
    /// You, then the other racer.
    r: [Racer; 2],
    obs: Vec<Ob>,
    rng: Rng,
    /// Since the countdown started.
    clock: u32,
    beeped: u32,
    /// Time since you crossed the finish line.
    cheer: Option<u32>,
    terrain: Terrain,
    flash: Option<(String, u32)>,
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
    if from == From::Title || snowy(g) {
        v.push(Kind::Sled);
    }
    v
}

pub fn render_games(g: &Game, f: &mut Frame, from: From) {
    ui::header(f, "Fun and games");
    let names: Vec<&str> = games(g, from).iter().map(|k| k.name()).collect();
    let mut y = ui::menu(f, 3, 18, &names) + 4;
    y = ui::para(
        f,
        3,
        y,
        "Just for fun! Nothing on your trip changes.",
        gfx::GRAY,
    );
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
        YOU_COLOR,
    );
    ui::footer(f, "Pick a game, or ESC to go back.");
}

pub fn games_key(g: &mut Game, from: From, k: Key) -> Scene {
    if k == Key::Escape {
        return match from {
            From::Title => Scene::Title,
            From::Trail => Scene::SizeUp,
        };
    }
    let list = games(g, from);
    match ui::digit(k).filter(|&i| i <= list.len()) {
        Some(i) => {
            g.beep(587, 25);
            Scene::Race(Box::new(RaceState::new(g, list[i - 1], from)))
        }
        None => Scene::Games(from),
    }
}

impl RaceState {
    fn new(g: &Game, kind: Kind, from: From) -> Self {
        let mut rng = Rng::new(g.rng.state() ^ (u64::from(g.clock) << 20) ^ kind as u64);
        // On the trail you race someone from your party, if anyone's along (not in a wagon race:
        // they're riding in yours).
        let party: Vec<&str> = g.t.party[1.min(g.t.party.len())..]
            .iter()
            .filter(|p| !p.gone)
            .map(|p| p.name.as_str())
            .collect();
        let rival = if from == From::Trail && kind != Kind::Wagon && !party.is_empty() {
            party[rng.below(party.len() as u32) as usize]
        } else {
            RIVALS[rng.below(RIVALS.len() as u32) as usize]
        };
        let mut obs = Vec::new();
        let len = kind.len();
        let mut at = 150;
        while at < len - 60 {
            let ob = |side, thing| Ob {
                at,
                side,
                thing,
                hit: [false; 2],
            };
            match kind {
                Kind::Wagon => {
                    let lane = rng.range(0, LANES - 1);
                    let thing = [Thing::Rock, Thing::Bump, Thing::Tree][rng.below(3) as usize];
                    obs.push(ob(lane, thing));
                    if rng.chance(300) {
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
        let mut r: [Racer; 2] = Default::default();
        match kind {
            Kind::Wagon => {
                r[0].side = 2;
                r[1].side = 0;
            }
            Kind::Sled => {
                r[0].side = 90 * FP;
                r[1].side = 140 * FP;
            }
            Kind::Foot => {}
        }
        for racer in &mut r {
            racer.y = lane_y(racer.side) * FP;
            racer.plan = racer.side;
        }
        RaceState {
            kind,
            from,
            rival: String::from(rival),
            r,
            obs,
            rng,
            clock: 0,
            beeped: 0,
            cheer: None,
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
    56 + lane * 20
}

/// The lane a wagon is nearest to while it slides over.
fn lane_at(y: i32) -> i32 {
    ((y / FP - lane_y(0) + 10) / 20).clamp(0, LANES - 1)
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
    if s.clock < START {
        return Scene::Race(s);
    }
    let you = &mut s.r[0];
    match s.kind {
        Kind::Wagon => {
            let d = up_down(k);
            if d != 0 {
                you.side = (you.side + d).clamp(0, LANES - 1);
            } else if matches!(k, Key::Char(' ') | Key::Enter) && you.hurt == 0 {
                hurry(you);
                you.shout = 350;
                g.beep(140, 15);
            }
        }
        Kind::Foot => {
            if (matches!(k, Key::Char(' ') | Key::Enter) || up_down(k) < 0) && jump(you) {
                g.tune(&[(523, 30), (784, 40)]);
            }
        }
        // Steering and going faster are keys held down (see `tick`).
        Kind::Sled => {}
    }
    Scene::Race(s)
}

pub fn tick(g: &mut Game, mut s: Box<RaceState>, ms: u32) -> Scene {
    s.clock += ms;
    while s.beeped < 4 && s.clock >= s.beeped * START / 3 {
        g.beep(if s.beeped == 3 { 880 } else { 440 }, 150);
        s.beeped += 1;
    }
    if s.clock < START {
        return Scene::Race(s);
    }
    if let Some(t) = &mut s.cheer {
        *t += ms;
    }
    let dt = ms as i32;
    for r in &mut s.r {
        r.hurt = r.hurt.saturating_sub(ms);
        r.shout = r.shout.saturating_sub(ms);
        r.bounce = r.bounce.saturating_sub(ms);
    }
    let said = match s.kind {
        Kind::Wagon => wagon_tick(&mut s, dt),
        Kind::Foot => foot_tick(&mut s, dt),
        Kind::Sled => sled_tick(g, &mut s, dt),
    };
    if let Some(said) = said {
        s.flash = Some((String::from(said), 1200));
        g.tune(&[(150, 60), (100, 120)]);
    }
    let now = s.clock - START;
    let len = s.kind.len();
    for i in 0..2 {
        if s.r[i].done.is_none() && s.r[i].pos / FP >= len {
            s.r[i].done = Some(now);
        }
    }
    // The race is over when either racer crosses the finish line.
    if s.r.iter().any(|r| r.done.is_some()) && s.cheer.is_none() {
        s.cheer = Some(0);
        if won(&s) {
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

fn won(s: &RaceState) -> bool {
    match (s.r[0].done, s.r[1].done) {
        (Some(a), Some(b)) => a <= b,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Moves both wagons. Returns what to say if you hit something.
fn wagon_tick(s: &mut RaceState, dt: i32) -> Option<&'static str> {
    let RaceState { r, obs, rng, .. } = s;
    // The other driver hurries the oxen now and then, and usually steers around trouble.
    let them = &mut r[1];
    them.think = them.think.saturating_sub(dt as u32);
    if them.think == 0 {
        hurry(them);
        them.think = rng.range(480, 720) as u32;
    }
    let front = them.pos / FP;
    if let Some(i) = obs
        .iter()
        .position(|o| o.side == them.side && o.at > front && o.at - front < 70)
        && them.seen != Some(i)
    {
        them.seen = Some(i);
        let clear = |lane: i32| {
            (0..LANES).contains(&lane)
                && !obs
                    .iter()
                    .any(|o| o.side == lane && (o.at - obs[i].at).abs() < 40)
        };
        let ways: Vec<i32> = [them.side - 1, them.side + 1]
            .into_iter()
            .filter(|&l| clear(l))
            .collect();
        if !ways.is_empty() && rng.chance(800) {
            them.side = ways[rng.below(ways.len() as u32) as usize];
        }
    }
    let mut said = None;
    for (i, r) in r.iter_mut().enumerate() {
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
            if ob.hit[i] || ob.side != lane || ob.at > front + w - 2 || ob.at < front - 36 {
                continue;
            }
            ob.hit[i] = true;
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
            if i == 0 {
                said = Some(what);
            }
        }
    }
    said
}

fn foot_tick(s: &mut RaceState, dt: i32) -> Option<&'static str> {
    let RaceState { r, obs, rng, .. } = s;
    // The other runner jumps each log or rock, though not always in time.
    let them = &mut r[1];
    let x = them.pos / FP;
    if let Some(i) = obs.iter().position(|o| o.at + 4 > x) {
        if them.seen != Some(i) {
            them.seen = Some(i);
            them.plan = if rng.chance(820) {
                rng.range(10, 28)
            } else {
                0
            };
        }
        if them.plan > 0 && obs[i].at - x <= them.plan {
            jump(them);
        }
    }
    let mut said = None;
    for (i, r) in r.iter_mut().enumerate() {
        let run = if i == 0 { RUN } else { RIVAL_RUN };
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
            if ob.hit[i] || ob.at >= x + 6 || ob.at + w <= x + 2 || r.up / FP >= h {
                continue;
            }
            ob.hit[i] = true;
            r.hurt = 900;
            if i == 0 {
                said = Some(if ob.thing == Thing::Log {
                    "Oops! Tripped on a log."
                } else {
                    "Oops! Tripped on a rock."
                });
            }
        }
    }
    said
}

fn sled_tick(g: &Game, s: &mut RaceState, dt: i32) -> Option<&'static str> {
    let RaceState { r, obs, rng, .. } = s;
    // The other sledder steers around what's ahead, usually.
    let them = &mut r[1];
    let (x, y) = (them.side / FP, them.pos / FP);
    if let Some(i) = obs
        .iter()
        .position(|o| o.at > y + 8 && o.at - y < 80 && (o.side - x).abs() < 13)
        && them.seen != Some(i)
    {
        them.seen = Some(i);
        let ox = obs[i].side;
        let to = if !rng.chance(850) {
            x
        } else if ox < 30 || (ox < W - 40 && x > ox) {
            ox + 15
        } else {
            ox - 15
        };
        them.plan = to * FP;
    }
    let mut said = None;
    for (i, r) in r.iter_mut().enumerate() {
        let (steer, (cap, accel)) = if i == 0 {
            let fast = g.held.down || g.held.fire;
            (
                i32::from(g.held.right) - i32::from(g.held.left),
                if fast { (125, 55) } else { (90, 35) },
            )
        } else {
            let d = r.plan - r.side;
            (if d.abs() > 2 * FP { d.signum() } else { 0 }, (106, 45))
        };
        if r.hurt > 0 {
            r.speed = 0;
        } else {
            let step = accel * FP * dt / 1000;
            r.speed = if r.speed < cap * FP {
                (r.speed + step).min(cap * FP)
            } else {
                (r.speed - step).max(cap * FP)
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
            if ob.hit[i] || x + 9 <= ox || x + 1 >= ox + bw || y + 10 <= oy || y + 3 >= oy + bh {
                continue;
            }
            ob.hit[i] = true;
            r.hurt = 1100;
            if i == 0 {
                said = Some(if ob.thing == Thing::Pine {
                    "Crash! Into a tree!"
                } else {
                    "Crash! Into a rock!"
                });
            }
        }
    }
    said
}

fn finish(g: &mut Game, s: &RaceState) -> Scene {
    let (name, what) = (&s.rival, s.kind.name().to_ascii_lowercase());
    let secs = |a: u32, b: u32| {
        let n = (a.abs_diff(b) + 500) / 1000;
        if n <= 1 {
            String::from("just a second")
        } else {
            format!("{n} seconds")
        }
    };
    let body = match (s.r[0].done, s.r[1].done) {
        (Some(a), Some(b)) if b < a => {
            format!("{name} won the {what} by {}. Want a rematch?", secs(a, b))
        }
        (Some(a), Some(b)) => format!("You won the {what} by {}!", secs(a, b)),
        (Some(_), None) => format!("You won the {what}! You beat {name} to the finish line."),
        _ => format!("{name} won the {what} this time. Want a rematch?"),
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
    let title = if won(s) { "You won!" } else { "Good race!" };
    g.say(title, format!("{body}\n\nDid you know? {fact}"));
    Scene::Games(s.from)
}

// ---------------------------------------------------------------- drawing

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
    if s.cheer.is_some() {
        let word = if won(s) {
            String::from("You win!")
        } else {
            format!("{} wins!", s.rival)
        };
        let w = word.len() as i32 * 10;
        let c = if won(s) { gfx::YELLOW } else { gfx::WHITE };
        f.label((W - w) / 2, 22, &word, c, gfx::BLACK, Font::Big);
        if s.cheer >= Some(CHEER_MS) {
            f.label(
                84,
                H - 11,
                "Press ENTER",
                gfx::WHITE,
                gfx::BLACK,
                Font::Normal,
            );
        }
    }
}

/// The top bar: a little track from start to finish with both racers on it.
fn hud(f: &mut Frame, s: &RaceState) {
    f.rect(0, 0, W, TOP, gfx::BLACK);
    let left = match s.kind {
        // Pixels a second to miles an hour, rounded: 48 is 3 mph.
        Kind::Wagon => format!("{} mph", (s.r[0].speed / FP + 8) / 16),
        _ => {
            let t = s.r[0].done.unwrap_or(s.clock.saturating_sub(START));
            format!("{}s", t / 1000)
        }
    };
    f.text(3, 2, &left, gfx::WHITE, Font::Small);
    let (x0, x1, len) = (52, 226, s.kind.len());
    f.rect(x0, 6, x1 - x0, 1, gfx::GRAY);
    checkers(f, x1, 2, 6, 8);
    for (i, c) in [(1, RIVAL_COLOR), (0, YOU_COLOR)] {
        let x = x0 + (x1 - x0) * (s.r[i].pos / FP).min(len) / len;
        f.rect(x - 2, 3, 5, 7, c);
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

fn name_tag(f: &mut Frame, x: i32, y: i32, name: &str, c: Color) {
    let w = name.len() as i32 * 5;
    f.label(x - w / 2, y, name, gfx::WHITE, c, Font::Small);
}

fn render_wagon(g: &Game, f: &mut Frame, s: &RaceState) {
    let you = s.r[0].pos / FP;
    pictures::backdrop(f, s.terrain, TOP, 50, you, gfx::SKY);
    f.rect(0, 52, W, 64, gfx::rgb(170, 130, 80));
    f.rect(0, 116, W, H - 116, pictures::ground_color(s.terrain));
    for l in 1..LANES {
        let y = lane_y(l) - 4;
        for k in 0..12 {
            let x = (k * 24 + you).rem_euclid(W + 24) - 24;
            f.rect(x, y, 10, 1, gfx::rgb(200, 165, 110));
        }
    }
    // Things ahead (farther along) are to the left: the trail goes west.
    let sx = |at: i32| TEAM_X - (at - you);
    let finish = sx(s.kind.len());
    if finish > -10 && finish < W {
        checkers(f, finish, 52, 4, 64);
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
        for (i, r) in s.r.iter().enumerate().rev() {
            if lane_at(r.y) != lane {
                continue;
            }
            let mut x = sx(r.pos / FP);
            let mut y = r.y / FP;
            if r.bounce > 0 {
                y -= ((r.bounce / 100) % 2) as i32 * 2;
            }
            if r.hurt > 0 {
                x += ((g.clock / 60) % 2) as i32;
            }
            let step = (r.pos / FP / 6) as usize;
            pictures::team(f, x, y, step);
            let c = if i == 0 { YOU_COLOR } else { RIVAL_COLOR };
            f.rect(x + 30, y - 6, 1, 6, gfx::DARK_BROWN);
            f.rect(x + 31, y - 6, 6, 4, c);
            if i == 1 {
                name_tag(f, x + 29, y - 15, &s.rival, RIVAL_COLOR);
            } else if r.shout > 0 {
                f.label(x - 2, y - 4, "Hyah!", gfx::YELLOW, gfx::BLACK, Font::Small);
            }
        }
    }
    edge_marker(
        f,
        sx(s.r[1].pos / FP),
        lane_y(lane_at(s.r[1].y)) + 4,
        true,
        40,
    );
}

/// When the other racer (`size` pixels long) is off the screen, show which way they are.
fn edge_marker(f: &mut Frame, pos: i32, across: i32, sideways: bool, size: i32) {
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
    f.rect(x, y, 5, 5, RIVAL_COLOR);
    f.outline(x - 1, y - 1, 7, 7, gfx::WHITE);
}

fn render_foot(g: &Game, f: &mut Frame, s: &RaceState) {
    let you = s.r[0].pos / FP;
    pictures::backdrop(f, s.terrain, TOP, 46, -you, gfx::SKY);
    f.rect(0, 58, W, H - 58, pictures::ground_color(s.terrain));
    let sx = |at: i32| RUN_X + (at - you);
    for track in [RIVAL_TRACK, YOUR_TRACK] {
        f.rect(0, track, W, 5, gfx::rgb(170, 130, 80));
        for ob in &s.obs {
            let x = sx(ob.at);
            if !(-20..W + 4).contains(&x) {
                continue;
            }
            if ob.thing == Thing::Log {
                f.rect(x, track - 7, 16, 7, gfx::BROWN);
                f.rect(x, track - 7, 16, 1, gfx::DARK_BROWN);
                f.rect(x, track - 1, 16, 1, gfx::DARK_BROWN);
                f.rect(x + 12, track - 7, 4, 7, gfx::TAN);
                f.rect(x + 13, track - 5, 2, 3, gfx::BROWN);
            } else {
                f.sprite(x, track - 6, &sprites::ROCK, false);
            }
        }
        let fin = sx(s.kind.len());
        if fin > -6 && fin < W {
            checkers(f, fin, track - 16, 4, 16);
        }
    }
    for (i, (r, track, c)) in [
        (&s.r[1], RIVAL_TRACK, RIVAL_COLOR),
        (&s.r[0], YOUR_TRACK, YOU_COLOR),
    ]
    .into_iter()
    .enumerate()
    {
        let x = sx(r.pos / FP);
        let y = track - 11 - r.up / FP;
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
        f.sprite_accent(x, y + wobble, &sprites::RUNNER[frame], false, c);
        if i == 0 {
            name_tag(f, x + 4, y - 9, &s.rival, RIVAL_COLOR);
            edge_marker(f, x, track - 9, true, 8);
        }
    }
}

fn render_sled(g: &Game, f: &mut Frame, s: &RaceState) {
    let you = s.r[0].pos / FP;
    f.rect(0, TOP, W, H - TOP, gfx::rgb(235, 240, 250));
    for i in 0..45 {
        let y = (i * 29 - you).rem_euclid(H - TOP) + TOP;
        let x = (i * 53 + i * i) % W;
        f.rect(x, y, 3, 1, gfx::rgb(195, 205, 230));
    }
    // Trees along both sides of the run.
    for i in 0..8 {
        let y = (i * 26 - you).rem_euclid(H - TOP + 12) + TOP - 12;
        f.sprite(-3 + (i % 2) * 2, y, &sprites::PINE, false);
        f.sprite(W - 7 - (i % 2) * 2, y + 13, &sprites::PINE, false);
    }
    // Things farther down the hill are lower on the screen.
    let sy = |at: i32| SLED_Y + (at - you);
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
    for (i, r) in s.r.iter().enumerate().rev() {
        let (x, y) = (r.side / FP, sy(r.pos / FP));
        let c = if i == 0 { YOU_COLOR } else { RIVAL_COLOR };
        if r.hurt > 0 {
            // Tumbled into the snow.
            for k in 0..6 {
                let a = (g.clock / 70) as i32 + k * 3;
                f.rect(x + (a * 7) % 14 - 2, y + (a * 5) % 10, 2, 2, gfx::WHITE);
            }
        }
        if r.hurt == 0 || (g.clock / 120) % 2 == 0 {
            f.sprite_accent(x, y, &sprites::SLEDDER, false, c);
        }
        if r.speed > 100 * FP && r.hurt == 0 {
            for k in 0..3 {
                f.rect(x + 2 + k * 3, y - 6, 1, 4, gfx::rgb(170, 185, 220));
            }
        }
        if i == 1 {
            name_tag(f, x + 5, y - 10, &s.rival, RIVAL_COLOR);
        }
    }
    edge_marker(f, sy(s.r[1].pos / FP), s.r[1].side / FP + 2, false, 10);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Plays one race from the title screen. `good` plays carefully; otherwise
    /// nobody touches the keys. Returns whether you won.
    fn race(seed: u64, pick: char, good: bool) -> bool {
        let mut g = Game::new(seed);
        g.key_down(Key::Char('6'));
        g.key_down(Key::Char(pick));
        assert!(matches!(g.scene, Scene::Race(_)));
        for step in 0..40_000u32 {
            if !g.msgs.is_empty() {
                return g.msgs[0].title == "You won!";
            }
            let mut press = None;
            if let Scene::Race(s) = &g.scene
                && s.cheer >= Some(CHEER_MS)
            {
                press = Some(Key::Enter);
            } else if good && let Scene::Race(s) = &g.scene {
                let you = &s.r[0];
                let (x, pos) = (you.side / FP, you.pos / FP);
                match s.kind {
                    Kind::Wagon => {
                        let lane = lane_at(you.y);
                        let blocked = |l: i32| {
                            s.obs
                                .iter()
                                .any(|o| o.side == l && o.at > pos - 30 && o.at - pos < 40)
                        };
                        if blocked(lane) {
                            let to = (0..LANES).find(|&l| (l - lane).abs() == 1 && !blocked(l));
                            press = to.map(|l| if l < lane { Key::Up } else { Key::Down });
                        } else if step % 20 == 0 {
                            press = Some(Key::Char(' '));
                        }
                    }
                    Kind::Foot => {
                        if s.obs.iter().any(|o| o.at - pos > 9 && o.at - pos <= 14) {
                            press = Some(Key::Char(' '));
                        }
                    }
                    Kind::Sled => {
                        g.held.fire = true;
                        let ahead = s
                            .obs
                            .iter()
                            .find(|o| o.at > pos && o.at - pos < 45 && (o.side - x).abs() < 14);
                        g.held.left = ahead.is_some_and(|o| o.side >= x && x > 20);
                        g.held.right = ahead.is_some_and(|o| !(o.side >= x && x > 20));
                    }
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

    #[test]
    fn careful_racers_win_and_idle_ones_lose() {
        for pick in ['1', '2', '3'] {
            let wins = (1..=6).filter(|&seed| race(seed, pick, true)).count();
            assert!(
                wins >= 5,
                "game {pick}: a careful player won only {wins} of 6"
            );
            let wins = (1..=3).filter(|&seed| race(seed, pick, false)).count();
            assert_eq!(wins, 0, "game {pick}: sitting still won");
        }
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
