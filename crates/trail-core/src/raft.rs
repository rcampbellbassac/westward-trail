//! Rafting down the Columbia River: steer left and right, miss the rocks.

use alloc::{boxed::Box, format, string::String, vec::Vec};

use crate::{
    Game, Key, Mood, Scene,
    gfx::{self, Font, Frame, H, W},
    river, sprites, travel,
};

const FP: i32 = 16;
const RIVER_LEN: i32 = 1800;
const SCROLL_SPEED: i32 = 42;
const STEER_SPEED: i32 = 80;
const RAFT_Y: i32 = 96;

pub struct RaftState {
    x: i32,
    dist: i32,
    rocks: Vec<(i32, i32)>,
    hits: u32,
    safe_ms: i32,
    lost: Vec<String>,
}

/// Left and right river banks at a distance down the river.
fn banks(world_y: i32) -> (i32, i32) {
    let wave = |p: i32, a: i32| {
        let t = world_y.rem_euclid(p);
        let tri = if t < p / 2 { t } else { p - t };
        tri * a * 2 / p - a / 2
    };
    let center = 120 + wave(500, 50) + wave(170, 14);
    let half = 60 + wave(330, 20);
    (center - half, center + half)
}

impl RaftState {
    pub fn new(g: &mut Game) -> Self {
        let mut rocks = Vec::new();
        let mut y = 160;
        while y < RIVER_LEN - 40 {
            let (l, r) = banks(y);
            rocks.push((g.rng.range(l + 4, r - 12), y));
            y += g.rng.range(28, 60);
        }
        let (l, r) = banks(0);
        RaftState {
            x: ((l + r) / 2 - 5) * FP,
            dist: 0,
            rocks,
            hits: 0,
            safe_ms: 0,
            lost: Vec::new(),
        }
    }
}

impl RaftState {
    /// Robot players: (raft x, the x it would be safest to steer to).
    #[cfg(test)]
    pub(crate) fn bot_target(&self) -> (i32, i32) {
        let rw = sprites::RAFT.w();
        let here = self.dist / FP;
        let ahead: Vec<i32> = self
            .rocks
            .iter()
            .filter(|&&(_, y)| y > here - 6 && y < here + 60)
            .map(|&(x, _)| x)
            .collect();
        let (l1, r1) = banks(here);
        let (l2, r2) = banks(here + 40);
        let (l, r) = (l1.max(l2) + 2, r1.min(r2) - rw - 2);
        let rx = self.x / FP;
        let mut best = (i32::MIN, rx);
        let mut x = l;
        while x <= r.max(l) {
            let rock = ahead
                .iter()
                .map(|&k| (k + 4 - (x + rw / 2)).abs())
                .min()
                .unwrap_or(99);
            let bank = (x - l).min(r - x) * 2;
            let score = rock.min(30) * 4 + bank.min(20) - (x - rx).abs() / 4;
            if score > best.0 {
                best = (score, x);
            }
            x += 2;
        }
        (rx, best.1)
    }
}

/// The river scrolls downward: things farther along appear at the top.
fn screen_y(s: &RaftState, world_y: i32) -> i32 {
    RAFT_Y - (world_y - s.dist / FP)
}

fn crash(g: &mut Game, s: &mut RaftState) {
    s.hits += 1;
    s.safe_ms = 1200;
    g.beep(100, 200);
    let food = g.t.sup.food * g.rng.range(5, 15) as u32 / 100;
    g.t.sup.food -= food;
    if food > 0 {
        s.lost.push(format!("{food} lbs of food"));
    }
    let (after, odds) = match g.t.mood {
        Mood::Gentle => (u32::MAX, 0),
        Mood::Classic => (3, 300),
        Mood::Survival => (2, 400),
    };
    if s.hits >= after && g.rng.chance(odds) {
        let alive: Vec<usize> = (0..g.t.party.len())
            .filter(|&i| !g.t.party[i].gone)
            .collect();
        if let Some(&i) = alive.get(g.rng.below(alive.len().max(1) as u32) as usize) {
            travel::lose_person(g, i, "drowning");
        }
    }
}

pub fn tick(g: &mut Game, mut s: Box<RaftState>, ms: u32) -> Scene {
    let dt = ms as i32;
    s.dist += SCROLL_SPEED * dt * FP / 1000;
    let steer = i32::from(g.held.right) - i32::from(g.held.left);
    s.x += steer * STEER_SPEED * dt * FP / 1000;
    s.safe_ms = (s.safe_ms - dt).max(0);

    let raft_world = s.dist / FP;
    let (l, r) = banks(raft_world);
    let rx = s.x / FP;
    let rw = sprites::RAFT.w();
    if rx < l || rx + rw > r {
        s.x = s.x.clamp(l * FP, (r - rw) * FP);
        if s.safe_ms == 0 {
            crash(g, &mut s);
        }
    }
    if s.safe_ms == 0 {
        let hit = s.rocks.iter().any(|&(x, y)| {
            let sy = screen_y(&s, y);
            sy + 6 > RAFT_Y && sy < RAFT_Y + 9 && x + 9 > rx && x < rx + rw
        });
        if hit {
            crash(g, &mut s);
        }
    }

    if s.dist / FP >= RIVER_LEN {
        let body = if s.hits == 0 {
            String::from("You steered past every rock. What a ride!")
        } else if s.lost.is_empty() {
            format!(
                "You bumped into rocks {} times, but nothing fell out.",
                s.hits
            )
        } else {
            format!(
                "You bumped into rocks {} times and lost {}.",
                s.hits,
                s.lost.join(", ")
            )
        };
        g.say("Columbia River", body);
        if g.t.alive_count() == 0 {
            return crate::score::game_over(g);
        }
        return river::finish_trail(g);
    }
    Scene::Raft(s)
}

pub fn render(g: &Game, f: &mut Frame, s: &RaftState) {
    f.clear(gfx::rgb(60, 120, 60));
    let top_world = s.dist / FP + RAFT_Y;
    for sy in 15..H {
        let wy = top_world - sy;
        let (l, r) = banks(wy);
        f.rect(l, sy, r - l, 1, gfx::BLUE);
        if (wy + (g.clock / 50) as i32) % 9 == 0 {
            f.rect(
                l + ((wy * 7).rem_euclid((r - l - 8).max(1))),
                sy,
                6,
                1,
                gfx::SKY,
            );
        }
    }
    for &(x, y) in &s.rocks {
        let sy = screen_y(s, y);
        if sy > 4 && sy < H {
            f.sprite(x, sy, &sprites::RIVER_ROCK, false);
        }
    }
    let blink = s.safe_ms > 0 && (g.clock / 100) % 2 == 0;
    if !blink {
        f.sprite(s.x / FP, RAFT_Y, &sprites::RAFT, false);
    }
    f.rect(0, 0, W, 15, gfx::BLACK);
    let left = (RIVER_LEN - s.dist / FP).max(0) * 100 / RIVER_LEN;
    f.text(
        3,
        2,
        &format!("Bumps {}   {left}% to go", s.hits),
        gfx::WHITE,
        Font::Small,
    );
    if s.dist / FP < 120 {
        f.banner(H - 17, "Steer: LEFT and RIGHT", gfx::WHITE, gfx::BLACK);
    }
}

#[allow(dead_code)]
pub fn key(_g: &mut Game, s: Box<RaftState>, _k: Key) -> Scene {
    Scene::Raft(s)
}
