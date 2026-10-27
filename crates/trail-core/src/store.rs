//! The general store at the start and the forts along the way.

use alloc::{format, string::String};

use crate::{
    Game, Key, Pic, Scene,
    data::Kind,
    gfx::{self, Font, Frame},
    ui,
};

pub struct StoreState {
    /// false = the first store before leaving; true = a fort or town on the trail.
    pub on_trail: bool,
    pub sel: Option<usize>,
    pub buf: String,
}

impl StoreState {
    pub fn new(on_trail: bool) -> Self {
        StoreState {
            on_trail,
            sel: None,
            buf: String::new(),
        }
    }
}

const ITEMS: [&str; 7] = [
    "Oxen",
    "Food",
    "Clothing",
    "Ammunition",
    "Wagon wheels",
    "Wagon axles",
    "Wagon tongues",
];
const MAX_FOOD: u32 = 2000;
const MAX_PARTS: u32 = 3;

/// Prices rise the farther west you go: up to double at the end of the trail.
fn price(g: &Game, item: usize) -> u32 {
    let p = &g.data.prices;
    let base = match item {
        0 => p.oxen,
        1 => p.food,
        2 => p.clothing,
        3 => p.ammo,
        _ => p.part,
    };
    let markup = 100 + g.t.miles * 100 / g.data.trail_length();
    base.saturating_mul(markup) / 100
}

fn unit(item: usize) -> &'static str {
    match item {
        0 => "yoke (2 oxen)",
        1 => "pound",
        2 => "set",
        3 => "box (20 bullets)",
        _ => "part",
    }
}

fn owned(g: &Game, item: usize) -> String {
    let s = &g.t.sup;
    match item {
        0 => format!("{} oxen", s.oxen),
        1 => format!("{} lbs", s.food),
        2 => format!("{} sets", s.clothing),
        3 => format!("{} bullets", s.ammo),
        4 => format!("{}", s.wheels),
        5 => format!("{}", s.axles),
        _ => format!("{}", s.tongues),
    }
}

fn advice(item: usize) -> &'static str {
    match item {
        0 => "There are 2 oxen in a yoke. I recommend at least 3 yoke.",
        1 => "I recommend at least 200 pounds of food for each person.",
        2 => "Take at least 2 sets of clothing per person for the cold mountains.",
        3 => "Each box holds 20 bullets for hunting.",
        _ => "A spare part can save you days if something breaks.",
    }
}

fn store_name(g: &Game, s: &StoreState) -> String {
    if s.on_trail {
        let at =
            g.t.at
                .and_then(|i| g.data.landmarks.get(i))
                .map_or("Trading post", |l| l.name.as_str());
        format!("{at} store")
    } else {
        g.data.prices.store_name.clone()
    }
}

pub fn render(g: &Game, f: &mut Frame, s: &StoreState) {
    ui::header(f, &store_name(g, s));
    match s.sel {
        None => {
            let mut y = 15;
            for (i, name) in ITEMS.iter().enumerate() {
                f.text(
                    3,
                    y,
                    &format!("{}. {name}", i + 1),
                    gfx::WHITE,
                    Font::Normal,
                );
                f.text(108, y, &owned(g, i), gfx::GRAY, Font::Normal);
                f.text(186, y, &ui::money(price(g, i)), gfx::YELLOW, Font::Normal);
                y += 10;
            }
            f.text(
                3,
                y + 4,
                &format!("Cash: {}", ui::money(g.t.sup.cash)),
                gfx::GREEN,
                Font::Normal,
            );
            ui::footer(f, "Number to buy.  ENTER when you're done.");
        }
        Some(i) => {
            f.text(3, 16, ITEMS[i], gfx::YELLOW, Font::Bold);
            let mut y = ui::para(f, 3, 32, advice(i), gfx::GRAY);
            let p = price(g, i);
            f.text(
                3,
                y + 2,
                &format!("{} per {}", ui::money(p), unit(i)),
                gfx::WHITE,
                Font::Normal,
            );
            y += 12;
            let afford = g.t.sup.cash.checked_div(p).unwrap_or(9999);
            f.text(
                3,
                y,
                &format!("You have {}, can afford {afford}", owned(g, i)),
                gfx::WHITE,
                Font::Normal,
            );
            let cursor = if (g.clock / 400) % 2 == 0 { "_" } else { " " };
            f.text(
                3,
                y + 16,
                &format!("How many? {}{cursor}", s.buf),
                gfx::YELLOW,
                Font::Normal,
            );
            ui::footer(f, "Type a number and ENTER.  ESC: go back.");
        }
    }
}

pub fn key(g: &mut Game, mut s: StoreState, k: Key) -> Scene {
    match s.sel {
        None => {
            if let Some(i) = ui::digit(k).filter(|i| *i <= ITEMS.len()) {
                s.sel = Some(i - 1);
                s.buf.clear();
                return Scene::Store(s);
            }
            if matches!(k, Key::Enter | Key::Escape) {
                return leave(g, s);
            }
        }
        Some(i) => match k {
            Key::Char(c @ '0'..='9') if s.buf.len() < 5 => s.buf.push(c),
            Key::Backspace => {
                s.buf.pop();
            }
            Key::Escape => s.sel = None,
            Key::Enter => {
                let qty: u32 = s.buf.parse().unwrap_or(0);
                buy(g, i, qty);
                s.sel = None;
            }
            _ => {}
        },
    }
    Scene::Store(s)
}

fn buy(g: &mut Game, item: usize, qty: u32) {
    if qty == 0 {
        return;
    }
    let sup = &g.t.sup;
    let (have, max) = match item {
        1 => (sup.food, MAX_FOOD),
        4 => (sup.wheels, MAX_PARTS),
        5 => (sup.axles, MAX_PARTS),
        6 => (sup.tongues, MAX_PARTS),
        _ => (0, u32::MAX),
    };
    if have.saturating_add(qty) > max {
        let what = if item == 1 {
            "pounds of food"
        } else {
            "of each spare part"
        };
        g.say(
            "Too much!",
            format!("Your wagon can only carry {max} {what}."),
        );
        return;
    }
    let cost = price(g, item).saturating_mul(qty);
    if cost > g.t.sup.cash {
        g.say(
            "Not enough money",
            format!(
                "That costs {}. You only have {}.",
                ui::money(cost),
                ui::money(g.t.sup.cash)
            ),
        );
        return;
    }
    let sup = &mut g.t.sup;
    sup.cash -= cost;
    match item {
        0 => sup.oxen = sup.oxen.saturating_add(qty * 2),
        1 => sup.food += qty,
        2 => sup.clothing = sup.clothing.saturating_add(qty),
        3 => sup.ammo = sup.ammo.saturating_add(qty.saturating_mul(20)),
        4 => sup.wheels += qty,
        5 => sup.axles += qty,
        _ => sup.tongues += qty,
    }
    g.tune(&[(784, 40), (1047, 60)]);
}

fn leave(g: &mut Game, s: StoreState) -> Scene {
    if s.on_trail {
        return Scene::SizeUp;
    }
    if g.t.sup.oxen == 0 {
        g.say(
            "Wait!",
            "Don't forget, you'll need oxen to pull your wagon.",
        );
        return Scene::Store(s);
    }
    let start = g.data.landmarks[0].name.clone();
    g.say_pic(
        start,
        "Well then, you're ready to start. Good luck! You have a long and difficult journey ahead of you.",
        Pic::Landmark(0),
    );
    g.t.at = Some(0);
    g.t.next = 1;
    debug_assert!(g.data.landmarks[0].kind != Kind::End);
    g.tune(&[(523, 120), (659, 120), (784, 120), (1047, 240)]);
    Scene::SizeUp
}
