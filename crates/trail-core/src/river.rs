//! River crossings, and the big choice at The Dalles.

use alloc::{boxed::Box, format, string::String, vec::Vec};

use crate::{
    Game, Key, Mood, Scene,
    data::Kind,
    fish,
    gfx::{self, Font, Frame, W},
    raft, score, sprites,
    travel::{self, DayMode},
    ui,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Method {
    Ford,
    Float,
    Ferry,
    Guide,
}

pub struct RiverState {
    pub idx: usize,
    /// Tenths of a foot.
    pub depth10: u32,
    pub width: u32,
    pub crossing: Option<(Method, u32)>,
}

const CROSS_MS: u32 = 2600;

impl RiverState {
    pub fn new(g: &mut Game, idx: usize) -> Self {
        let l = &g.data.landmarks[idx];
        let (base_depth, base_width) = (l.depth10, l.width);
        let season = match ui::month_index(g) {
            2..=4 => 120,
            6..=8 => 80,
            _ => 100,
        };
        let depth10 = base_depth * season / 100 * g.rng.range(80, 120) as u32 / 100;
        let width = base_width * g.rng.range(90, 110) as u32 / 100;
        RiverState {
            idx,
            depth10: depth10.max(5),
            width,
            crossing: None,
        }
    }
}

#[derive(Clone, Copy)]
enum Opt {
    Ford,
    Float,
    Ferry(u32),
    Guide,
    Wait,
    Info,
    Fish,
}

fn options(g: &Game, r: &RiverState) -> Vec<(Opt, String)> {
    let l = &g.data.landmarks[r.idx];
    let mut v = alloc::vec![
        (Opt::Ford, String::from("Attempt to ford the river")),
        (Opt::Float, String::from("Caulk the wagon and float")),
    ];
    if l.ferry > 0 {
        v.push((
            Opt::Ferry(l.ferry),
            format!("Take a ferry for ${}", l.ferry),
        ));
    }
    if l.guide {
        v.push((
            Opt::Guide,
            String::from("Hire a guide (3 sets of clothing)"),
        ));
    }
    v.push((Opt::Wait, String::from("Wait to see if it gets better")));
    v.push((Opt::Info, String::from("Get more information")));
    v.push((Opt::Fish, String::from("Go fishing first")));
    v
}

fn depth_text(d: u32) -> String {
    format!("{}.{} feet", d / 10, d % 10)
}

pub fn render(g: &Game, f: &mut Frame, r: &RiverState) {
    let l = &g.data.landmarks[r.idx];
    if let Some((method, ms)) = r.crossing {
        render_crossing(g, f, method, ms, &l.name);
        return;
    }
    ui::header(f, &l.name);
    f.text(
        3,
        14,
        &format!("{}   {}", ui::date(g), ui::weather_word(g)),
        gfx::GRAY,
        Font::Normal,
    );
    f.text(
        3,
        24,
        &format!("Width {} feet, depth {}", r.width, depth_text(r.depth10)),
        gfx::WHITE,
        Font::Normal,
    );
    let mut y = 38;
    for (i, (_, label)) in options(g, r).iter().enumerate() {
        f.text(
            3,
            y,
            &format!("{}. {label}", i + 1),
            gfx::WHITE,
            Font::Normal,
        );
        y += 10;
    }
    ui::footer(f, "What is your choice?");
}

fn render_crossing(g: &Game, f: &mut Frame, method: Method, ms: u32, name: &str) {
    f.clear(gfx::SKY);
    f.rect(0, 70, W, 65, gfx::BLUE);
    f.rect(0, 62, 30, 73, gfx::rgb(110, 170, 70));
    f.rect(210, 62, 30, 73, gfx::rgb(110, 170, 70));
    for i in 0..24 {
        let x = (i * 41 + (g.clock / 30) as i32) % W;
        f.rect(x, 76 + (i % 6) * 9, 7, 1, gfx::SKY);
    }
    let x = 10 + (190 * ms.min(CROSS_MS) / CROSS_MS) as i32;
    let bob = if method == Method::Float {
        ((g.clock / 200) % 2) as i32
    } else {
        0
    };
    if method == Method::Ferry {
        f.rect(x - 6, 76, 36, 5, gfx::DARK_BROWN);
    }
    pictures_team(f, x, 60 + bob);
    f.label(3, 2, name, gfx::WHITE, gfx::BLACK, Font::Small);
    let what = match method {
        Method::Ford => "Fording the river...",
        Method::Float => "Floating across...",
        Method::Ferry => "Riding the ferry...",
        Method::Guide => "Following your guide...",
    };
    f.text(3, 14, what, gfx::BLACK, Font::Normal);
}

fn pictures_team(f: &mut Frame, x: i32, y: i32) {
    f.sprite(x, y + 6, &sprites::OX[0], true);
    f.sprite(x + 14, y, &sprites::WAGON, true);
}

pub fn key(g: &mut Game, mut r: RiverState, k: Key) -> Scene {
    if r.crossing.is_some() {
        return Scene::River(r);
    }
    let opts = options(g, &r);
    let Some(i) = ui::digit(k).filter(|i| *i <= opts.len()) else {
        return Scene::River(r);
    };
    match opts[i - 1].0 {
        Opt::Ford => r.crossing = Some((Method::Ford, 0)),
        Opt::Float => r.crossing = Some((Method::Float, 0)),
        Opt::Ferry(cost) => {
            if g.t.sup.cash < cost * 100 {
                g.say("Ferry", "You don't have enough money for the ferry.");
            } else {
                let wait = g.rng.range(0, 3) as u32;
                g.t.sup.cash -= cost * 100;
                if wait > 0 {
                    g.say("Ferry", format!("The ferry is busy. You wait {wait} day{} for your turn.", if wait == 1 { "" } else { "s" }));
                    travel::pass_days(g, wait, DayMode::Busy);
                }
                r.crossing = Some((Method::Ferry, 0));
            }
        }
        Opt::Guide => {
            if g.t.sup.clothing < 3 {
                g.say("Guide", "The guide asks for 3 sets of clothing, but you don't have enough.");
            } else {
                g.t.sup.clothing -= 3;
                g.say("Guide", "A Shoshoni guide agrees to show you the safest way across.");
                r.crossing = Some((Method::Guide, 0));
            }
        }
        Opt::Wait => {
            travel::sim_day(g, DayMode::Rest);
            let change = g.rng.range(-8, 4);
            r.depth10 = (r.depth10 as i32 * (100 + change) / 100).max(5) as u32;
            g.say("Waiting", format!("You camp by the river for a day. It is now {} deep.", depth_text(r.depth10)));
            if g.t.alive_count() == 0 {
                return score::game_over(g);
            }
        }
        Opt::Info => g.say(
            "River crossings",
            "Fording: walk the oxen across. Safe if the river is under 2.5 feet deep, risky over 3 feet.\n\
             Caulking: seal the wagon so it floats like a boat. It might tip over.\n\
             A ferry or a guide is safest, but costs money or clothing.",
        ),
        Opt::Fish => {
            let mut s = fish::FishState::new();
            s.river = Some(r.idx);
            return Scene::Fish(Box::new(s));
        }
    }
    Scene::River(r)
}

pub fn tick(g: &mut Game, mut r: RiverState, ms: u32) -> Scene {
    let Some((method, t)) = r.crossing else {
        return Scene::River(r);
    };
    let t = t + ms;
    if t < CROSS_MS {
        r.crossing = Some((method, t));
        return Scene::River(r);
    }
    finish_crossing(g, &r, method);
    sim_day(g);
    if g.t.alive_count() == 0 {
        return score::game_over(g);
    }
    Scene::SizeUp
}

fn sim_day(g: &mut Game) {
    travel::sim_day(g, DayMode::Busy);
}

fn finish_crossing(g: &mut Game, r: &RiverState, method: Method) {
    let d = r.depth10;
    let title = g.data.landmarks[r.idx].name.clone();
    // permille chances: (mishap, someone might drown)
    let (bad, drown) = match method {
        Method::Ford if d < 25 => (0, 0),
        Method::Ford if d <= 30 => (300, 0),
        Method::Ford => ((200 + (d - 30) * 15).min(900), 300),
        Method::Float => (
            100 + if r.width > 600 { 50 } else { 0 } + if d > 50 { 100 } else { 0 },
            250,
        ),
        Method::Ferry => (30, 0),
        Method::Guide => (50, 0),
    };
    if !g.rng.chance(bad) {
        g.say(title, "You made it safely across the river!");
        g.tune(&[(523, 80), (659, 80), (784, 160)]);
        return;
    }
    if method == Method::Ford && d <= 30 {
        g.say(
            title,
            "Your supplies got wet. You lose 1 day drying them out.",
        );
        travel::pass_days(g, 1, DayMode::Busy);
        return;
    }
    let lost = lose_supplies(g);
    let what = if method == Method::Ford {
        "The river was too deep and the wagon was swamped!"
    } else {
        "The wagon tipped over in the river!"
    };
    g.say(title.clone(), format!("{what} You lost {lost}."));
    g.beep(110, 300);
    if g.t.mood == Mood::Classic && g.rng.chance(drown) {
        let alive: Vec<usize> = (0..g.t.party.len())
            .filter(|&i| !g.t.party[i].gone)
            .collect();
        if !alive.is_empty() {
            let i = alive[g.rng.below(alive.len() as u32) as usize];
            travel::lose_person(g, i, "drowning");
        }
    } else if g.t.mood == Mood::Gentle {
        g.say(
            title,
            "Everyone got soaking wet, but they all swam to shore safely.",
        );
    }
}

/// Wash away part of the supplies and describe what was lost.
fn lose_supplies(g: &mut Game) -> String {
    let mut lost = Vec::new();
    let food = g.t.sup.food * g.rng.range(10, 40) as u32 / 100;
    if food > 0 {
        g.t.sup.food -= food;
        lost.push(format!("{food} pounds of food"));
    }
    let clothes = g.t.sup.clothing.min(g.rng.range(0, 2) as u32);
    if clothes > 0 {
        g.t.sup.clothing -= clothes;
        lost.push(format!("{clothes} sets of clothing"));
    }
    let ammo = g.t.sup.ammo.min(g.rng.range(0, 3) as u32 * 20);
    if ammo > 0 {
        g.t.sup.ammo -= ammo;
        lost.push(format!("{ammo} bullets"));
    }
    if g.t.sup.oxen > 1 && g.rng.chance(200) {
        g.t.sup.oxen -= 1;
        lost.push(String::from("1 ox"));
    }
    if lost.is_empty() {
        return String::from("a few small things");
    }
    lost.join(", ")
}

// ---------------------------------------------------------------- The Dalles

fn toll(g: &Game) -> u32 {
    g.t.at
        .and_then(|i| g.data.landmarks.get(i))
        .map_or(30, |l| l.toll)
}

pub fn render_dalles(g: &Game, f: &mut Frame) {
    let name =
        g.t.at
            .and_then(|i| g.data.landmarks.get(i))
            .map_or("The Dalles", |l| l.name.as_str());
    ui::header(f, name);
    let y = ui::para(
        f,
        3,
        16,
        "The trail ends here. To reach the valley you must choose:",
        gfx::GRAY,
    );
    let road = format!("Take the Barlow Road (${})", toll(g));
    let y = ui::menu(
        f,
        3,
        y + 4,
        &[
            "Raft down the Columbia River",
            &road,
            "Size up the situation",
        ],
    );
    f.text(
        3,
        y + 6,
        &format!("Cash: {}", ui::money(g.t.sup.cash)),
        gfx::GREEN,
        Font::Normal,
    );
}

pub fn dalles_key(g: &mut Game, k: Key) -> Scene {
    match ui::digit(k) {
        Some(1) => Scene::Raft(Box::new(raft::RaftState::new(g))),
        Some(2) => {
            let cost = toll(g) * 100;
            if g.t.sup.cash < cost {
                g.say(
                    "Barlow Road",
                    "You don't have enough money for the toll. You'll have to raft.",
                );
                return Scene::Dalles;
            }
            g.t.sup.cash -= cost;
            let days = g.rng.range(8, 12) as u32;
            travel::pass_days(g, days, DayMode::Travel);
            if g.t.alive_count() == 0 {
                return score::game_over(g);
            }
            g.say(
                "Barlow Road",
                format!("After {days} days of steep, muddy road over the mountain, you made it!"),
            );
            finish_trail(g)
        }
        Some(3) => Scene::SizeUp,
        _ => Scene::Dalles,
    }
}

/// Jump to the last landmark (the end of the trail).
pub fn finish_trail(g: &mut Game) -> Scene {
    let end = g
        .data
        .landmarks
        .iter()
        .position(|l| l.kind == Kind::End)
        .unwrap_or(g.data.landmarks.len() - 1);
    travel::arrive_at(g, end)
}
