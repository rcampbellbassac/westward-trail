//! Fishing: cast your line, wait for a bite, and press SPACE at the right moment.

use alloc::{boxed::Box, format, string::String};

use crate::{
    Game, Key, Scene,
    gfx::{self, Font, Frame, H, W},
    river, sprites,
    travel::{self, DayMode},
};

const FISH_MS: u32 = 40_000;
const WATER_TOP: i32 = 40;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Stage {
    Ready,
    /// Waiting for a bite; counts down.
    Waiting(i32),
    /// A fish is biting! Counts down your chance.
    Bite(i32),
    /// Showing what you caught (or missed).
    Result(i32),
}

pub struct FishState {
    stage: Stage,
    elapsed: u32,
    lbs: u32,
    caught: u32,
    last: Option<(usize, u32)>,
    note: String,
    cast_x: i32,
    /// Come back to this river's crossing menu when done.
    pub river: Option<usize>,
}

impl Default for FishState {
    fn default() -> Self {
        Self::new()
    }
}

impl FishState {
    pub fn new() -> Self {
        FishState {
            stage: Stage::Ready,
            elapsed: 0,
            lbs: 0,
            caught: 0,
            last: None,
            note: String::from("Press SPACE to cast your line."),
            cast_x: 140,
            river: None,
        }
    }
}

pub fn key(g: &mut Game, mut s: Box<FishState>, k: Key) -> Scene {
    if k == Key::Escape {
        return finish(g, &s);
    }
    if !matches!(k, Key::Char(' ') | Key::Enter) {
        return Scene::Fish(s);
    }
    match s.stage {
        Stage::Ready | Stage::Result(_) => {
            s.cast_x = g.rng.range(90, 200);
            s.stage = Stage::Waiting(g.rng.range(1500, 5000));
            s.note = String::from("Wait for a bite...");
            g.beep(440, 30);
        }
        Stage::Waiting(_) => {
            s.stage = Stage::Result(1200);
            s.note = String::from("Too soon! You scared the fish away.");
            s.last = None;
        }
        Stage::Bite(_) => {
            let weights: alloc::vec::Vec<u32> = g
                .data
                .fish
                .iter()
                .map(|f| if f.max_lbs == 0 { 2 } else { 10 })
                .collect();
            match g.rng.weighted(&weights) {
                Some(i) => {
                    let fsh = &g.data.fish[i];
                    let (lo, hi, name) = (fsh.min_lbs, fsh.max_lbs, fsh.name.clone());
                    let lbs = g.rng.range(lo as i32, hi as i32) as u32;
                    s.lbs += lbs;
                    if lbs > 0 {
                        s.caught += 1;
                        s.note = format!("You caught a {lbs} pound {name}!");
                    } else {
                        s.note = format!("You caught an {name}. Yuck!");
                    }
                    s.last = Some((i, lbs));
                    g.tune(&[(523, 60), (784, 60), (1047, 100)]);
                }
                None => s.note = String::from("Nothing seems to live in this river..."),
            }
            s.stage = Stage::Result(1800);
        }
    }
    Scene::Fish(s)
}

pub fn tick(g: &mut Game, mut s: Box<FishState>, ms: u32) -> Scene {
    s.elapsed += ms;
    if s.elapsed >= FISH_MS {
        g.say("Fishing", "The sun is going down. Time to head back.");
        return finish(g, &s);
    }
    let dt = ms as i32;
    s.stage = match s.stage {
        Stage::Waiting(t) if t - dt <= 0 => {
            g.tune(&[(880, 40), (0, 30), (880, 40)]);
            s.note = String::from("A BITE! Press SPACE!");
            Stage::Bite(if g.t.mood == crate::Mood::Gentle {
                1100
            } else {
                750
            })
        }
        Stage::Waiting(t) => Stage::Waiting(t - dt),
        Stage::Bite(t) if t - dt <= 0 => {
            s.note = String::from("It got away! Press SPACE to cast again.");
            s.last = None;
            Stage::Result(1500)
        }
        Stage::Bite(t) => Stage::Bite(t - dt),
        Stage::Result(t) => Stage::Result((t - dt).max(0)),
        Stage::Ready => Stage::Ready,
    };
    Scene::Fish(s)
}

fn finish(g: &mut Game, s: &FishState) -> Scene {
    g.t.sup.food += s.lbs;
    let body = if s.lbs == 0 {
        String::from("You didn't catch anything to eat today.")
    } else {
        format!(
            "You caught {} fish, {} pounds of food in all!",
            s.caught, s.lbs
        )
    };
    g.say("Fishing", body);
    travel::sim_day(g, DayMode::Busy);
    if g.t.alive_count() == 0 {
        return crate::score::game_over(g);
    }
    match s.river {
        Some(idx) => Scene::River(river::RiverState::new(g, idx)),
        None => Scene::SizeUp,
    }
}

pub fn render(g: &Game, f: &mut Frame, s: &FishState) {
    f.clear(gfx::SKY);
    f.rect(0, WATER_TOP, W, H - WATER_TOP, gfx::BLUE);
    for i in 0..30 {
        let x = (i * 37 + (g.clock / 40) as i32) % W;
        f.rect(
            x,
            WATER_TOP + 4 + (i * 11) % (H - WATER_TOP - 6),
            6,
            1,
            gfx::rgb(120, 170, 240),
        );
    }
    // The bank and the fisher.
    f.rect(
        0,
        WATER_TOP - 6,
        50,
        H - WATER_TOP + 6,
        gfx::rgb(110, 170, 70),
    );
    f.sprite_accent(30, WATER_TOP - 16, &sprites::HUNTER, false, gfx::RED);
    let rod_tip = (52, WATER_TOP - 22);
    for i in 0..16 {
        f.pixel(36 + i, WATER_TOP - 10 - i * 12 / 16, gfx::DARK_BROWN);
    }
    if s.stage != Stage::Ready {
        let bob_y = WATER_TOP
            + 30
            + if matches!(s.stage, Stage::Bite(_)) {
                3
            } else {
                0
            };
        for i in 0..20 {
            let x = rod_tip.0 + (s.cast_x - rod_tip.0) * i / 20;
            let y = rod_tip.1 + (bob_y - rod_tip.1) * i / 20;
            f.pixel(x, y, gfx::WHITE);
        }
        f.rect(s.cast_x - 1, bob_y - 2, 3, 2, gfx::RED);
        f.rect(s.cast_x - 1, bob_y, 3, 2, gfx::WHITE);
        if matches!(s.stage, Stage::Bite(_)) && (g.clock / 120) % 2 == 0 {
            f.text(s.cast_x - 2, bob_y - 16, "!", gfx::YELLOW, Font::Bold);
        }
    }
    if let (Stage::Result(_), Some((i, _))) = (s.stage, s.last) {
        let sprite = if g.data.fish[i].max_lbs == 0 {
            &sprites::BOOT
        } else {
            &sprites::FISH
        };
        f.sprite(110, 18, sprite, false);
    }
    f.rect(0, 0, W, 12, gfx::BLACK);
    let left = FISH_MS.saturating_sub(s.elapsed) / 1000;
    f.text(
        3,
        2,
        &format!("Fish {}   Food {} lbs   Time {left}", s.caught, s.lbs),
        gfx::WHITE,
        Font::Small,
    );
    f.label(4, H - 12, &s.note, gfx::WHITE, gfx::BLACK, Font::Normal);
}
