//! Hunting: walk around, aim, and shoot before time runs out.
//! Positions are "fixed point": 16 units = 1 pixel, so slow things move smoothly.

use alloc::{format, string::String, vec::Vec};

use crate::{
    Game, Key, Scene,
    data::Terrain,
    gfx::{self, Font, Frame, H, W},
    pictures, sprites,
    travel::{self, DayMode},
};

const FP: i32 = 16;
const TOP: i32 = 12;
const HUNT_MS: u32 = 30_000;
const CARRY: u32 = 100;
const HUNTER_SPEED: i32 = 55;
const BULLET_SPEED: i32 = 220;

struct Beast {
    kind: usize,
    x: i32,
    y: i32,
    vx: i32,
    vy: i32,
    dead: bool,
    fleeing: bool,
}

struct Bullet {
    x: i32,
    y: i32,
    dx: i32,
    dy: i32,
    life: i32,
}

#[derive(Clone, Copy)]
enum Thing {
    Tree,
    Bush,
    Rock,
}

pub struct HuntState {
    x: i32,
    y: i32,
    face: (i32, i32),
    beasts: Vec<Beast>,
    bullets: Vec<Bullet>,
    things: Vec<(i32, i32, Thing)>,
    elapsed: u32,
    spawn_in: i32,
    lbs: u32,
    flash: Option<(String, u32)>,
    terrain: Terrain,
    step: u32,
}

fn thing_box(x: i32, y: i32, t: Thing) -> (i32, i32, i32, i32) {
    match t {
        Thing::Tree => (x + 1, y, 9, 13),
        Thing::Bush => (x, y, 8, 5),
        Thing::Rock => (x, y + 1, 10, 5),
    }
}

fn overlaps(a: (i32, i32, i32, i32), b: (i32, i32, i32, i32)) -> bool {
    a.0 < b.0 + b.2 && b.0 < a.0 + a.2 && a.1 < b.1 + b.3 && b.1 < a.1 + a.3
}

impl HuntState {
    pub fn new(g: &mut Game) -> Self {
        let terrain = travel::terrain(g);
        let kinds: &[Thing] = match terrain {
            Terrain::Forest => &[Thing::Tree, Thing::Tree, Thing::Tree, Thing::Bush],
            Terrain::Plains => &[Thing::Bush, Thing::Tree, Thing::Rock],
            Terrain::Hills => &[Thing::Tree, Thing::Rock, Thing::Bush],
            Terrain::Mountains => &[Thing::Rock, Thing::Rock, Thing::Tree],
            Terrain::Desert => &[Thing::Bush, Thing::Rock],
        };
        let mut things = Vec::new();
        let count = g.rng.range(6, 10);
        while (things.len() as i32) < count {
            let x = g.rng.range(4, W - 14);
            let y = g.rng.range(TOP + 4, H - 16);
            if (x - 116).abs() < 24 && (y - 64).abs() < 24 {
                continue; // keep the middle clear for the hunter
            }
            things.push((x, y, kinds[g.rng.below(kinds.len() as u32) as usize]));
        }
        HuntState {
            x: 116 * FP,
            y: 64 * FP,
            face: (16, 0),
            beasts: Vec::new(),
            bullets: Vec::new(),
            things,
            elapsed: 0,
            spawn_in: 600,
            lbs: 0,
            flash: None,
            terrain,
            step: 0,
        }
    }

    fn blocked(&self, x: i32, y: i32) -> bool {
        let me = (x / FP + 1, y / FP + 6, 6, 6);
        if me.0 < 0 || me.0 + me.2 > W || me.1 < TOP || me.1 + me.3 > H {
            return true;
        }
        self.things
            .iter()
            .any(|&(tx, ty, t)| overlaps(me, thing_box(tx, ty, t)))
    }
}

fn spawn(g: &mut Game, h: &mut HuntState) {
    let weights: Vec<u32> = g
        .data
        .animals
        .iter()
        .map(|a| {
            if a.terrain.is_empty() || a.terrain.contains(&h.terrain) {
                a.rarity
            } else {
                0
            }
        })
        .collect();
    let Some(kind) = g.rng.weighted(&weights) else {
        return;
    };
    let a = &g.data.animals[kind];
    let speed = a.speed as i32;
    let sw = sprites::animal(&a.sprite).w();
    let from_left = g.rng.below(2) == 0;
    let y = g.rng.range(TOP + 8, H - 16);
    h.beasts.push(Beast {
        kind,
        x: if from_left { -sw * FP } else { W * FP },
        y: y * FP,
        vx: if from_left { speed } else { -speed },
        vy: g.rng.range(-8, 8),
        dead: false,
        fleeing: false,
    });
}

pub fn key(g: &mut Game, mut h: alloc::boxed::Box<HuntState>, k: Key) -> Scene {
    match k {
        Key::Escape => return finish(g, &h),
        Key::Char(' ') | Key::Enter => {
            if g.t.sup.ammo == 0 && !g.cheats.infinite_ammo {
                h.flash = Some((String::from("Out of bullets!"), 1200));
                g.beep(120, 60);
            } else {
                if !g.cheats.infinite_ammo {
                    g.t.sup.ammo -= 1;
                }
                let (cx, cy) = (h.x + 4 * FP, h.y + 6 * FP);
                h.bullets.push(Bullet {
                    x: cx,
                    y: cy,
                    dx: h.face.0,
                    dy: h.face.1,
                    life: 700,
                });
                g.beep(90, 40);
                for b in h.beasts.iter_mut().filter(|b| !b.dead) {
                    if (b.x - cx).abs() < 90 * FP && (b.y - cy).abs() < 60 * FP && !b.fleeing {
                        b.fleeing = true;
                        b.vx = if b.x > cx { b.vx.abs() } else { -b.vx.abs() } * 17 / 10;
                    }
                }
            }
        }
        _ => {}
    }
    Scene::Hunt(h)
}

pub fn tick(g: &mut Game, mut h: alloc::boxed::Box<HuntState>, ms: u32) -> Scene {
    let dt = ms as i32;
    h.elapsed += ms;
    if h.elapsed >= HUNT_MS {
        g.say("Hunting", "Time's up!");
        return finish(g, &h);
    }

    // Move the hunter, one axis at a time so you can slide along trees.
    let held = g.held;
    let dx = i32::from(held.right) - i32::from(held.left);
    let dy = i32::from(held.down) - i32::from(held.up);
    if dx != 0 || dy != 0 {
        h.face = match (dx, dy) {
            (0, _) => (0, dy * 16),
            (_, 0) => (dx * 16, 0),
            _ => (dx * 11, dy * 11),
        };
        let step = HUNTER_SPEED * dt * FP / 1000;
        let nx = h.x + dx * step;
        if !h.blocked(nx, h.y) {
            h.x = nx;
        }
        let ny = h.y + dy * step;
        if !h.blocked(h.x, ny) {
            h.y = ny;
        }
        h.step += ms;
    }

    // Animals.
    h.spawn_in -= dt;
    if h.spawn_in <= 0 {
        if h.beasts.iter().filter(|b| !b.dead).count() < 4 {
            spawn(g, &mut h);
        }
        h.spawn_in = g.rng.range(1200, 2600);
    }
    for b in h.beasts.iter_mut().filter(|b| !b.dead) {
        b.x += b.vx * dt * FP / 1000;
        b.y += b.vy * dt * FP / 1000;
        if b.y < (TOP + 4) * FP || b.y > (H - 14) * FP {
            b.vy = -b.vy;
        }
    }
    if g.rng.chance(30) {
        for b in h.beasts.iter_mut().filter(|b| !b.dead) {
            b.vy = g.rng.range(-12, 12);
        }
    }
    h.beasts
        .retain(|b| b.dead || (b.x > -40 * FP && b.x < (W + 40) * FP));

    // Bullets fly until they hit a tree, an animal, or run out of range.
    let mut hits: Vec<usize> = Vec::new();
    let animals = &g.data.animals;
    let HuntState {
        bullets,
        beasts,
        things,
        ..
    } = &mut *h;
    bullets.retain_mut(|bl| {
        bl.x += bl.dx * BULLET_SPEED * dt / 1000;
        bl.y += bl.dy * BULLET_SPEED * dt / 1000;
        bl.life -= dt;
        let p = (bl.x / FP, bl.y / FP, 2, 2);
        if bl.life <= 0
            || things
                .iter()
                .any(|&(x, y, t)| overlaps(p, thing_box(x, y, t)))
        {
            return false;
        }
        let hit = beasts.iter().enumerate().position(|(i, b)| {
            let s = sprites::animal(&animals[b.kind].sprite);
            !b.dead && !hits.contains(&i) && overlaps(p, (b.x / FP, b.y / FP, s.w(), s.h()))
        });
        match hit {
            Some(i) => {
                hits.push(i);
                false
            }
            None => true,
        }
    });
    for i in hits {
        let a = &g.data.animals[h.beasts[i].kind];
        let (name, lo, hi) = (a.name.clone(), a.min_lbs, a.max_lbs);
        let lbs = g.rng.range(lo as i32, hi as i32) as u32;
        h.beasts[i].dead = true;
        h.lbs += lbs;
        h.flash = Some((format!("You got a {name}! +{lbs} lbs"), 1500));
        g.tune(&[(330, 50), (494, 80)]);
    }
    if let Some((_, t)) = &mut h.flash {
        *t = t.saturating_sub(ms);
        if *t == 0 {
            h.flash = None;
        }
    }
    Scene::Hunt(h)
}

fn finish(g: &mut Game, h: &HuntState) -> Scene {
    let body = if h.lbs == 0 {
        String::from("You didn't get anything this time. Better luck next time!")
    } else if h.lbs > CARRY {
        format!(
            "From the animals you shot, you got {} pounds of meat. But you could only carry {CARRY} pounds back to the wagon.",
            h.lbs
        )
    } else {
        format!(
            "From the animals you shot, you got {} pounds of meat.",
            h.lbs
        )
    };
    g.t.sup.food += h.lbs.min(CARRY);
    g.say("Hunting", body);
    travel::sim_day(g, DayMode::Busy);
    if g.t.alive_count() == 0 {
        return crate::score::game_over(g);
    }
    Scene::SizeUp
}

pub fn render(g: &Game, f: &mut Frame, h: &HuntState) {
    f.rect(0, TOP, W, H - TOP, pictures::ground_color(h.terrain));
    for i in 0..30 {
        f.rect(
            (i * 53) % W,
            TOP + (i * 31) % (H - TOP),
            2,
            1,
            gfx::rgb(70, 110, 50),
        );
    }
    for &(x, y, t) in &h.things {
        let s = match t {
            Thing::Tree => &sprites::TREE,
            Thing::Bush => &sprites::BUSH,
            Thing::Rock => &sprites::ROCK,
        };
        f.sprite(x, y, s, false);
    }
    for b in &h.beasts {
        let s = sprites::animal(&g.data.animals[b.kind].sprite);
        if b.dead {
            f.sprite_ex(b.x / FP, b.y / FP, s, b.vx < 0, true, None);
        } else {
            f.sprite(b.x / FP, b.y / FP, s, b.vx < 0);
        }
    }
    let (hx, hy) = (h.x / FP, h.y / FP);
    f.sprite(hx, hy, &sprites::HUNTER, h.face.0 < 0);
    let (cx, cy) = (hx + 4, hy + 6);
    for i in 2..8 {
        f.pixel(
            cx + h.face.0 * i / 16,
            cy + h.face.1 * i / 16,
            gfx::DARK_GRAY,
        );
    }
    for bl in &h.bullets {
        f.rect(bl.x / FP, bl.y / FP, 2, 2, gfx::BLACK);
    }
    f.rect(0, 0, W, TOP, gfx::BLACK);
    let left = (HUNT_MS.saturating_sub(h.elapsed)) / 1000;
    let ammo = if g.cheats.infinite_ammo {
        String::from("lots")
    } else {
        format!("{}", g.t.sup.ammo)
    };
    f.text(
        3,
        2,
        &format!("Bullets {ammo}   Meat {} lbs   Time {left}", h.lbs),
        gfx::WHITE,
        Font::Small,
    );
    if let Some((msg, _)) = &h.flash {
        let w = msg.len() as i32 * 6;
        f.label((W - w) / 2, 16, msg, gfx::YELLOW, gfx::BLACK, Font::Normal);
    }
    if h.elapsed < 2500 {
        f.label(
            30,
            H - 12,
            "Move: arrows/WASD   Shoot: SPACE",
            gfx::WHITE,
            gfx::BLACK,
            Font::Small,
        );
    }
}
