//! Gathering: walk around and pick berries, mushrooms and roots.
//! What grows depends on where you are on the trail and the month, from the
//! [plant] sections of the trail file. Some plants are poisonous look-alikes.

use alloc::{format, string::String, vec::Vec};

use crate::{
    Game, Key, Mood, Scene,
    data::Terrain,
    gfx::{self, Font, Frame, H, W},
    pictures, sprites,
    travel::{self, DayMode},
    ui,
};

const FP: i32 = 16;
const TOP: i32 = 12;
const FORAGE_MS: u32 = 30_000;
const BASKET: u32 = 40;
const WALK_SPEED: i32 = 50;
/// How close (in pixels) you have to be to pick something.
const REACH: i32 = 12;

struct Patch {
    plant: usize,
    x: i32,
    y: i32,
    /// Handfuls left before it's picked clean.
    left: u32,
}

pub struct ForageState {
    x: i32,
    y: i32,
    face_left: bool,
    patches: Vec<Patch>,
    /// Pounds gathered, per plant.
    got: Vec<(usize, u32)>,
    /// Poisonous plants that went in the basket without anyone noticing.
    bad: Vec<usize>,
    elapsed: u32,
    flash: Option<(String, u32)>,
    terrain: Terrain,
}

/// Month 1-12 right now.
fn month(g: &Game) -> u32 {
    ui::month_index(g) as u32 + 1
}

/// Plants that grow here this month.
fn in_season(g: &Game) -> Vec<usize> {
    let (mile, m) = (g.t.miles, month(g));
    g.data
        .plants
        .iter()
        .enumerate()
        .filter(|(_, p)| p.grows(mile, m))
        .map(|(i, _)| i)
        .collect()
}

/// Start gathering, or explain why there's nothing to find.
pub fn start(g: &mut Game) -> Scene {
    let here = in_season(g);
    if !here.iter().any(|&i| !g.data.plants[i].poison) {
        let m = ui::MONTHS[month(g) as usize - 1];
        g.say(
            "Gathering",
            format!("You look around, but nothing good to eat is growing here in {m}."),
        );
        return Scene::SizeUp;
    }
    if g.t.mood == Mood::Gentle && !g.forage_warned {
        g.forage_warned = true;
        g.say(
            "Gathering",
            "Pioneers picked wild berries, roots and mushrooms to eat.\n\nIn real life, never eat a wild plant unless a grown-up who really knows plants says it's safe. Some poisonous plants look just like good ones!",
        );
    }
    let weights: Vec<u32> = here.iter().map(|&i| g.data.plants[i].rarity).collect();
    let mut patches = Vec::new();
    let count = g.rng.range(9, 13);
    let mut tries = 0;
    while (patches.len() as i32) < count && tries < 200 {
        tries += 1;
        let x = g.rng.range(4, W - 16);
        let y = g.rng.range(TOP + 4, H - 14);
        if (x - 116).abs() < 18 && (y - 64).abs() < 18 {
            continue; // keep the middle clear for the forager
        }
        if patches
            .iter()
            .any(|p: &Patch| (p.x - x).abs() < 14 && (p.y - y).abs() < 10)
        {
            continue;
        }
        let Some(w) = g.rng.weighted(&weights) else {
            break;
        };
        patches.push(Patch {
            plant: here[w],
            x,
            y,
            left: g.rng.range(2, 4) as u32,
        });
    }
    Scene::Forage(alloc::boxed::Box::new(ForageState {
        x: 116 * FP,
        y: 64 * FP,
        face_left: false,
        patches,
        got: Vec::new(),
        bad: Vec::new(),
        elapsed: 0,
        flash: None,
        terrain: travel::terrain(g),
    }))
}

fn total(s: &ForageState) -> u32 {
    s.got.iter().map(|(_, lbs)| lbs).sum()
}

/// The plant within reach, if any.
fn nearest(g: &Game, s: &ForageState) -> Option<usize> {
    let (cx, cy) = (s.x / FP + 4, s.y / FP + 8);
    s.patches
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let sp = sprites::plant(&g.data.plants[p.plant].sprite);
            let (px, py) = (p.x + sp.w() / 2, p.y + sp.h() / 2);
            (i, (px - cx).abs() + (py - cy).abs())
        })
        .filter(|&(_, d)| d <= REACH + 6)
        .min_by_key(|&(_, d)| d)
        .map(|(i, _)| i)
}

fn pick(g: &mut Game, s: &mut ForageState) {
    let Some(i) = nearest(g, s) else {
        s.flash = Some((String::from("Walk up to a plant first."), 900));
        return;
    };
    let plant = &g.data.plants[s.patches[i].plant];
    let (name, poison, looks_like) = (plant.name.clone(), plant.poison, plant.looks_like.clone());
    if poison {
        // Little pioneers always get warned. Seasoned ones notice half the time.
        if g.t.mood == Mood::Gentle || g.rng.chance(500) {
            let like = if looks_like.is_empty() {
                String::new()
            } else {
                format!(" They look like {looks_like}.")
            };
            s.flash = Some((format!("Stop! That's {name}!{like}"), 2200));
            s.patches.remove(i);
            g.tune(&[(330, 60), (220, 120)]);
            s.elapsed += 1500; // it takes a moment to back away and look again
        } else {
            s.bad.push(s.patches[i].plant);
            s.patches[i].left -= 1;
            if s.patches[i].left == 0 {
                s.patches.remove(i);
            }
            s.flash = Some((String::from("You pick a handful."), 900));
            g.beep(523, 40);
        }
        return;
    }
    if total(s) >= BASKET {
        s.flash = Some((String::from("Your basket is full!"), 1200));
        g.beep(150, 60);
        return;
    }
    let (lo, hi) = plant.lbs;
    let lbs = (g.rng.range(lo as i32, hi as i32) as u32).min(BASKET - total(s));
    let kind = s.patches[i].plant;
    match s.got.iter_mut().find(|(k, _)| *k == kind) {
        Some((_, l)) => *l += lbs,
        None => s.got.push((kind, lbs)),
    }
    s.patches[i].left -= 1;
    if s.patches[i].left == 0 {
        s.patches.remove(i);
    }
    s.flash = Some((format!("{name}  +{lbs} lb"), 1100));
    g.tune(&[(523, 40), (784, 60)]);
}

pub fn key(g: &mut Game, mut s: alloc::boxed::Box<ForageState>, k: Key) -> Scene {
    match k {
        Key::Escape => return finish(g, &s),
        Key::Char(' ') | Key::Enter => pick(g, &mut s),
        _ => {}
    }
    Scene::Forage(s)
}

pub fn tick(g: &mut Game, mut s: alloc::boxed::Box<ForageState>, ms: u32) -> Scene {
    s.elapsed += ms;
    if s.elapsed >= FORAGE_MS {
        g.say(
            "Gathering",
            "The sun is getting low. Time to head back to the wagon.",
        );
        return finish(g, &s);
    }
    let held = g.held;
    let dx = i32::from(held.right) - i32::from(held.left);
    let dy = i32::from(held.down) - i32::from(held.up);
    if dx != 0 {
        s.face_left = dx < 0;
    }
    let step = WALK_SPEED * ms as i32 * FP / 1000;
    s.x = (s.x + dx * step).clamp(0, (W - 8) * FP);
    s.y = (s.y + dy * step).clamp(TOP * FP, (H - 12) * FP);
    if let Some((_, t)) = &mut s.flash {
        *t = t.saturating_sub(ms);
        if *t == 0 {
            s.flash = None;
        }
    }
    Scene::Forage(s)
}

fn list(g: &Game, items: &[(usize, u32)]) -> String {
    let parts: Vec<String> = items
        .iter()
        .map(|&(k, lbs)| format!("{lbs} lbs of {}", g.data.plants[k].name))
        .collect();
    match parts.len() {
        0 => String::new(),
        1 => parts[0].clone(),
        n => format!("{} and {}", parts[..n - 1].join(", "), parts[n - 1]),
    }
}

fn finish(g: &mut Game, s: &ForageState) -> Scene {
    let lbs = total(s);
    if lbs == 0 {
        g.say(
            "Gathering",
            "You came back with an empty basket. Maybe next time!",
        );
    } else {
        // Fresh fruit and greens kept scurvy away, so they help everyone's health a little.
        g.t.sup.food += lbs;
        g.t.health = g.t.health.saturating_sub((lbs / 4).min(10));
        let mut body = format!(
            "You gathered {}. That's {lbs} pounds of fresh food!",
            list(g, &s.got)
        );
        let pick = s.got[g.rng.below(s.got.len() as u32) as usize].0;
        let fact = &g.data.plants[pick].fact;
        if !fact.is_empty() {
            body.push_str(&format!("\n\nDid you know? {fact}"));
        }
        g.say("Gathering", body);
    }
    if let Some(&bad) = s.bad.first() {
        let plant = g.data.plants[bad].clone();
        let alive: Vec<usize> = (0..g.t.party.len())
            .filter(|&i| !g.t.party[i].gone)
            .collect();
        if !alive.is_empty() {
            let i = alive[g.rng.below(alive.len() as u32) as usize];
            // The truly deadly plants (death camas, water hemlock) rarely kill.
            if plant.deadly && g.rng.chance(200) {
                let mut body = format!("Some {} got mixed into your basket.", plant.name);
                if !plant.fact.is_empty() {
                    body.push_str(&format!("\n\n{}", plant.fact));
                }
                g.say("Poisoned", body);
                travel::lose_person(g, i, &format!("eating {}", plant.name));
                travel::sim_day(g, DayMode::Busy);
                if g.t.alive_count() == 0 {
                    return crate::score::game_over(g);
                }
                return Scene::SizeUp;
            }
            let name = g.t.party[i].name.clone();
            g.t.party[i].sick = Some((String::from("food poisoning"), 0));
            g.t.health += 15;
            let mut body = format!(
                "Some {} got mixed into your basket. {name} ate them and is very sick.",
                plant.name
            );
            if !plant.fact.is_empty() {
                body.push_str(&format!("\n\n{}", plant.fact));
            }
            g.say("Food poisoning", body);
            g.beep(220, 160);
        }
    }
    travel::sim_day(g, DayMode::Busy);
    if g.t.alive_count() == 0 {
        return crate::score::game_over(g);
    }
    Scene::SizeUp
}

pub fn render(g: &Game, f: &mut Frame, s: &ForageState) {
    f.rect(0, TOP, W, H - TOP, pictures::ground_color(s.terrain));
    for i in 0..40 {
        f.rect(
            (i * 47 + 11) % W,
            TOP + (i * 23) % (H - TOP),
            2,
            1,
            gfx::rgb(70, 110, 50),
        );
    }
    let near = nearest(g, s);
    for (i, p) in s.patches.iter().enumerate() {
        let plant = &g.data.plants[p.plant];
        let sp = sprites::plant(&plant.sprite);
        // A patch of darker ground so small plants stand out from the grass.
        f.rect(
            p.x - 2,
            p.y + sp.h() - 3,
            sp.w() + 4,
            4,
            gfx::rgb(60, 95, 40),
        );
        f.rect(
            p.x - 1,
            p.y + sp.h() - 4,
            sp.w() + 2,
            1,
            gfx::rgb(60, 95, 40),
        );
        let (r, gr, b) = plant.color;
        f.sprite_accent(p.x, p.y, sp, false, gfx::rgb(r, gr, b));
        if near == Some(i) {
            let bob = ((g.clock / 200) % 2) as i32;
            let ax = p.x + sp.w() / 2;
            for row in 0..3 {
                f.rect(
                    ax - 2 + row,
                    p.y - 6 + row + bob,
                    5 - row * 2,
                    1,
                    gfx::YELLOW,
                );
            }
            // Younger pioneers learn the names; seasoned ones have to know them.
            // Poisonous plants only get a subtle clue, so there's something to figure out.
            if g.t.mood == Mood::Gentle && plant.poison {
                let lines = gfx::wrap(&plant.hint, 46);
                let ly = if p.y > 60 {
                    p.y - 10 - lines.len() as i32 * 9
                } else {
                    p.y + sp.h() + 3
                };
                for (n, line) in lines.iter().enumerate() {
                    let w = line.len() as i32 * 5 + 4;
                    let lx = (ax - w / 2).clamp(1, W - w - 1);
                    f.label(
                        lx,
                        ly + n as i32 * 9,
                        line,
                        gfx::YELLOW,
                        gfx::rgb(40, 30, 10),
                        Font::Small,
                    );
                }
            } else if g.t.mood == Mood::Gentle {
                let w = plant.name.len() as i32 * 5 + 4;
                let lx = (ax - w / 2).clamp(1, W - w - 1);
                let ly = (p.y - 15).max(TOP + 1);
                f.label(
                    lx,
                    ly,
                    &plant.name,
                    gfx::WHITE,
                    gfx::rgb(20, 40, 15),
                    Font::Small,
                );
            }
        }
    }
    let (fx, fy) = (s.x / FP, s.y / FP);
    f.sprite(fx, fy, &sprites::GATHERER, s.face_left);

    f.rect(0, 0, W, TOP, gfx::BLACK);
    let left = FORAGE_MS.saturating_sub(s.elapsed) / 1000;
    f.text(
        3,
        2,
        &format!("Basket {}/{BASKET} lbs   Time {left}", total(s)),
        gfx::WHITE,
        Font::Small,
    );
    if let Some((msg, _)) = &s.flash {
        let lines = gfx::wrap(msg, 38);
        for (n, line) in lines.iter().enumerate() {
            let w = line.len() as i32 * 6;
            f.label(
                (W - w) / 2,
                16 + n as i32 * 11,
                line,
                gfx::YELLOW,
                gfx::BLACK,
                Font::Normal,
            );
        }
    }
    if s.elapsed < 2500 {
        f.label(
            34,
            H - 12,
            "Move: arrows/WASD   Pick: SPACE",
            gfx::WHITE,
            gfx::BLACK,
            Font::Small,
        );
    }
}
