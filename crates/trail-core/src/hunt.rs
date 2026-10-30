//! Hunting: walk around, aim, and shoot before time runs out.
//! Positions are "fixed point": 16 units = 1 pixel, so slow things move smoothly.
//!
//! Playing together, every player gets a hunter in the same field: the first
//! is this device's, the others move by the keys their players send.

use alloc::{format, string::String, vec::Vec};

use crate::{
    Game, Held, Key, Scene,
    data::Terrain,
    gfx::{self, Color, Font, Frame, H, W},
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
    /// Which hunter fired it.
    who: usize,
}

struct Hunter {
    /// The player's id when hunting together (0 alone).
    id: u32,
    name: String,
    color: Color,
    x: i32,
    y: i32,
    face: (i32, i32),
    step: u32,
    lbs: u32,
}

#[derive(Clone, Copy)]
enum Thing {
    Tree,
    Bush,
    Rock,
}

pub struct HuntState {
    hunters: Vec<Hunter>,
    beasts: Vec<Beast>,
    bullets: Vec<Bullet>,
    things: Vec<(i32, i32, Thing)>,
    elapsed: u32,
    spawn_in: i32,
    flash: Option<(String, u32)>,
    terrain: Terrain,
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
            if (x - 116).abs() < 44 && (y - 64).abs() < 28 {
                continue; // keep the middle clear for the hunters
            }
            things.push((x, y, kinds[g.rng.below(kinds.len() as u32) as usize]));
        }
        // Everyone playing together hunts; alone, it's just you in red.
        let mut hunters: Vec<Hunter> = Vec::new();
        if crate::party::hosting(g) {
            for p in &g.net.players {
                hunters.push(Hunter {
                    id: p.id,
                    name: p.name.clone(),
                    color: g.net.color(p.id),
                    x: 0,
                    y: 0,
                    face: (16, 0),
                    step: 0,
                    lbs: 0,
                });
            }
        }
        if hunters.is_empty() {
            hunters.push(Hunter {
                id: 0,
                name: String::from("You"),
                color: gfx::RED,
                x: 0,
                y: 0,
                face: (16, 0),
                step: 0,
                lbs: 0,
            });
        }
        // Side by side in the middle.
        let n = hunters.len() as i32;
        for (i, h) in hunters.iter_mut().enumerate() {
            h.x = (116 + (i as i32 * 2 - (n - 1)) * 10).clamp(4, W - 12) * FP;
            h.y = (64 + if i % 2 == 0 { 0 } else { 8 }) * FP;
        }
        HuntState {
            hunters,
            beasts: Vec::new(),
            bullets: Vec::new(),
            things,
            elapsed: 0,
            spawn_in: 600,
            flash: None,
            terrain,
        }
    }

    /// More than one hunter: playing together.
    pub fn together(&self) -> bool {
        self.hunters.len() > 1
    }

    /// Where each hunter stands, in pixels (for tests).
    #[cfg(test)]
    pub(crate) fn spots(&self) -> Vec<(i32, i32)> {
        self.hunters.iter().map(|h| (h.x / FP, h.y / FP)).collect()
    }

    fn lbs(&self) -> u32 {
        self.hunters.iter().map(|h| h.lbs).sum()
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
        // Hunting together the round runs its time, so nobody's hunt is cut short.
        Key::Escape if !h.together() => return finish(g, &h),
        Key::Char(' ') | Key::Enter => shoot(g, &mut h, 0),
        _ => {}
    }
    Scene::Hunt(h)
}

fn shoot(g: &mut Game, h: &mut HuntState, who: usize) {
    if g.t.sup.ammo == 0 && !g.cheats.infinite_ammo {
        h.flash = Some((String::from("Out of bullets!"), 1200));
        g.beep(120, 60);
        return;
    }
    if !g.cheats.infinite_ammo {
        g.t.sup.ammo -= 1;
    }
    let hunter = &h.hunters[who];
    let (cx, cy) = (hunter.x + 4 * FP, hunter.y + 6 * FP);
    h.bullets.push(Bullet {
        x: cx,
        y: cy,
        dx: hunter.face.0,
        dy: hunter.face.1,
        life: 700,
        who,
    });
    g.beep(90, 40);
    for b in h.beasts.iter_mut().filter(|b| !b.dead) {
        if (b.x - cx).abs() < 90 * FP && (b.y - cy).abs() < 60 * FP && !b.fleeing {
            b.fleeing = true;
            b.vx = if b.x > cx { b.vx.abs() } else { -b.vx.abs() } * 17 / 10;
        }
    }
}

/// Walk one hunter, one axis at a time so they can slide along trees.
fn walk(h: &mut HuntState, who: usize, held: Held, ms: u32) {
    let dx = i32::from(held.right) - i32::from(held.left);
    let dy = i32::from(held.down) - i32::from(held.up);
    if dx == 0 && dy == 0 {
        return;
    }
    let step = HUNTER_SPEED * ms as i32 * FP / 1000;
    let (x, y) = (h.hunters[who].x, h.hunters[who].y);
    let nx = x + dx * step;
    let nx = if h.blocked(nx, y) { x } else { nx };
    let ny = y + dy * step;
    let ny = if h.blocked(nx, ny) { y } else { ny };
    let me = &mut h.hunters[who];
    me.face = match (dx, dy) {
        (0, _) => (0, dy * 16),
        (_, 0) => (dx * 16, 0),
        _ => (dx * 11, dy * 11),
    };
    me.x = nx;
    me.y = ny;
    me.step += ms;
}

pub fn tick(g: &mut Game, mut h: alloc::boxed::Box<HuntState>, ms: u32) -> Scene {
    let dt = ms as i32;
    h.elapsed += ms;
    if h.elapsed >= HUNT_MS {
        g.say("Hunting", "Time's up!");
        return finish(g, &h);
    }

    // This device's hunter, then everyone else's by the keys they sent.
    walk(&mut h, 0, g.held, ms);
    for i in 1..h.hunters.len() {
        let held = g.net.party.held_of(h.hunters[i].id);
        walk(&mut h, i, held, ms);
    }
    for id in core::mem::take(&mut g.net.party.shots) {
        if let Some(i) = h.hunters.iter().position(|x| x.id == id) {
            shoot(g, &mut h, i);
        }
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
    let mut hits: Vec<(usize, usize)> = Vec::new();
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
            !b.dead
                && !hits.iter().any(|(j, _)| *j == i)
                && overlaps(p, (b.x / FP, b.y / FP, s.w(), s.h()))
        });
        match hit {
            Some(i) => {
                hits.push((i, bl.who));
                false
            }
            None => true,
        }
    });
    for (i, who) in hits {
        let a = &g.data.animals[h.beasts[i].kind];
        let (name, lo, hi) = (a.name.clone(), a.min_lbs, a.max_lbs);
        let lbs = g.rng.range(lo as i32, hi as i32) as u32;
        h.beasts[i].dead = true;
        let hunter = &mut h.hunters[who];
        hunter.lbs += lbs;
        let text = if h.hunters.len() > 1 {
            format!("{} got a {name}! +{lbs}", h.hunters[who].name)
        } else {
            format!("You got a {name}! +{lbs} lbs")
        };
        h.flash = Some((text, 1500));
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
    if g.net.racing && g.net.settings.mode == crate::net::Mode::Hunt {
        let results = h.hunters.iter().map(|x| (x.id, x.lbs)).collect();
        return crate::net::hunt_over(g, results);
    }
    // Each hunter carries their share back.
    let (lbs, carry) = (h.lbs(), CARRY * h.hunters.len() as u32);
    let mut body = if lbs == 0 {
        String::from("You didn't get anything this time. Better luck next time!")
    } else if lbs > carry {
        format!(
            "From the animals you shot, you got {lbs} pounds of meat. But you could only carry {carry} pounds back to the wagon."
        )
    } else {
        format!("From the animals you shot, you got {lbs} pounds of meat.")
    };
    if h.together() {
        for x in &h.hunters {
            body.push_str(&format!("\n{}: {} lbs", x.name, x.lbs));
        }
    }
    g.t.sup.food += lbs.min(carry);
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
    for hunter in &h.hunters {
        let (hx, hy) = (hunter.x / FP, hunter.y / FP);
        f.sprite_accent(hx, hy, &sprites::HUNTER, hunter.face.0 < 0, hunter.color);
        let (cx, cy) = (hx + 4, hy + 6);
        for i in 2..8 {
            f.pixel(
                cx + hunter.face.0 * i / 16,
                cy + hunter.face.1 * i / 16,
                gfx::DARK_GRAY,
            );
        }
        if h.together() {
            let initial: String = hunter.name.chars().take(1).collect();
            f.text(hx + 2, hy - 8, &initial, hunter.color, Font::Small);
        }
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
    let ammo = if g.net.racing && g.net.settings.mode == crate::net::Mode::Hunt {
        String::from("lots")
    } else {
        ammo
    };
    f.text(
        3,
        2,
        &format!("Bullets {ammo}   Meat {} lbs   Time {left}", h.lbs()),
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
