//! Moving down the trail: the daily simulation, random events, and the
//! "size up the situation" menu with everything you can do when stopped.

use alloc::{format, string::String, vec::Vec};
use embedded_graphics::{
    prelude::*,
    primitives::{Line, PrimitiveStyle},
};

use crate::{
    Game, Key, Mood, Pic, Precip, Scene,
    data::{Kind, Skill, Terrain},
    gfx::{self, Font, Frame, W},
    hunt, pictures, river, score, store, ui,
};

/// How long one trail day takes on screen, before the hacker speed boost.
const DAY_MS: u32 = 450;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DayMode {
    Travel,
    Rest,
    Busy,
}

// ---------------------------------------------------------------- helpers

pub fn terrain(g: &Game) -> Terrain {
    let i = g.t.at.unwrap_or(g.t.next.saturating_sub(1));
    g.data
        .landmarks
        .get(i)
        .map_or(Terrain::Plains, |l| l.terrain)
}

fn gentle(g: &Game) -> bool {
    g.t.mood == Mood::Gentle
}

fn skill(g: &Game) -> Skill {
    g.data
        .occupations
        .get(g.t.occupation)
        .map_or(Skill::None, |o| o.skill)
}

/// A random member of the party who is still with you.
fn someone(g: &mut Game) -> Option<usize> {
    let alive: Vec<usize> = (0..g.t.party.len())
        .filter(|&i| !g.t.party[i].gone)
        .collect();
    if alive.is_empty() {
        return None;
    }
    Some(alive[g.rng.below(alive.len() as u32) as usize])
}

fn pct(n: u32, p: u32) -> u32 {
    n.saturating_mul(p) / 100
}

// ---------------------------------------------------------------- the daily simulation

pub fn pass_days(g: &mut Game, n: u32, mode: DayMode) {
    for _ in 0..n {
        sim_day(g, mode);
    }
}

pub fn sim_day(g: &mut Game, mode: DayMode) {
    g.t.day += 1;
    g.t.days += 1;
    weather(g);

    let eaters = g.t.alive_count();
    let need = eaters * [3, 2, 1][g.t.rations.min(2)];
    let starving = g.t.sup.food < need;
    g.t.sup.food = g.t.sup.food.saturating_sub(need);

    // Health points: 0 is perfect. Each day some of it heals, then hardships add more.
    let mut add = [0, 2, 4][g.t.rations.min(2)];
    if mode == DayMode::Travel {
        add += [0, 3, 6][g.t.pace.min(2)];
        if g.t.precip != Precip::Clear {
            add += 1;
        }
    }
    if starving {
        add += 8;
    }
    let clothes = g.t.sup.clothing;
    if g.t.temp < 20 {
        add += if clothes >= eaters * 2 { 1 } else { 4 };
    } else if g.t.temp < 40 {
        add += if clothes >= eaters { 0 } else { 2 };
    } else if g.t.temp >= 90 {
        add += 2;
    }
    add += g.t.alive().filter(|p| p.sick.is_some()).count() as u32;
    let keep = if mode == DayMode::Rest { 75 } else { 88 };
    g.t.health = (pct(g.t.health, keep) + add).min(200);

    if g.t.ox_lame > 0 {
        g.t.ox_lame -= if mode == DayMode::Rest {
            2.min(g.t.ox_lame)
        } else {
            1
        };
    }
    illness(g);
}

fn weather(g: &mut Game) {
    const BASE: [i32; 12] = [25, 30, 40, 52, 62, 72, 80, 78, 68, 55, 40, 30];
    const WET: [u32; 12] = [150, 150, 200, 220, 200, 150, 100, 100, 120, 150, 180, 180];
    let m = ui::month_index(g);
    let t = terrain(g);
    let adj = match t {
        Terrain::Mountains => -15,
        Terrain::Desert => 10,
        Terrain::Forest => -5,
        Terrain::Hills => -3,
        Terrain::Plains => 0,
    };
    g.t.temp = BASE[m] + adj + g.rng.range(-8, 8);
    let wet = if t == Terrain::Desert {
        WET[m] * 2 / 5
    } else {
        WET[m]
    };
    g.t.precip = if !g.rng.chance(wet) {
        Precip::Clear
    } else if g.t.temp < 34 {
        Precip::Snow
    } else {
        Precip::Rain
    };
}

fn illness(g: &mut Game) {
    let h = g.t.health;
    let gentle = gentle(g);
    for i in 0..g.t.party.len() {
        if g.t.party[i].gone {
            continue;
        }
        if let Some((what, days)) = g.t.party[i].sick.clone() {
            let days = days + 1;
            g.t.party[i].sick = Some((what.clone(), days));
            let recover = 160u32.saturating_sub(h).max(40);
            if g.rng.chance(recover) {
                g.t.party[i].sick = None;
                let name = g.t.party[i].name.clone();
                g.say(ui::date(g), format!("{name} is feeling better."));
                continue;
            }
            let mut die = if h >= 70 {
                (h - 50) / 3
            } else if days > 15 {
                5
            } else {
                0
            };
            if gentle {
                die /= 2;
            }
            if g.rng.chance(die) {
                lose_person(g, i, &what);
            }
        } else {
            let mut odds = 2 + h / 5;
            if gentle {
                odds /= 2;
            }
            if g.rng.chance(odds) {
                let k = g.rng.below(g.data.illnesses.len() as u32) as usize;
                let ill = &g.data.illnesses[k];
                let what = if gentle {
                    ill.gentle.clone()
                } else {
                    ill.name.clone()
                };
                let name = g.t.party[i].name.clone();
                g.t.party[i].sick = Some((what.clone(), 0));
                g.say(ui::date(g), format!("{name} has {what}."));
                g.beep(220, 120);
            }
        }
    }
    if h >= 140
        && !gentle
        && g.rng.chance(10)
        && let Some(i) = someone(g)
    {
        lose_person(g, i, "exhaustion");
    }
}

/// Someone leaves the party: they die (classic) or get a ride home (gentle).
pub fn lose_person(g: &mut Game, i: usize, cause: &str) {
    if g.cheats.god {
        return;
    }
    let name = g.t.party[i].name.clone();
    g.t.party[i].gone = true;
    g.t.party[i].sick = None;
    if gentle(g) {
        g.say(
            "Going home",
            format!(
                "{name} is too sick to keep going, and gets a ride back home with a kind family heading east. \
                 They'll rest up and be just fine!"
            ),
        );
        g.tune(&[(392, 200), (330, 200), (392, 300)]);
    } else {
        g.say_pic(
            ui::date(g),
            format!("{name} has died of {cause}."),
            Pic::Tomb(name.clone()),
        );
        g.tune(&[
            (262, 300),
            (262, 200),
            (262, 100),
            (262, 300),
            (311, 200),
            (294, 100),
            (294, 200),
            (262, 100),
            (262, 200),
            (247, 100),
            (262, 400),
        ]);
    }
}

fn miles_today(g: &mut Game) -> u32 {
    let base = [190, 240, 290][g.t.pace.min(2)];
    let oxen = match g.t.sup.oxen {
        0 => 0,
        1 => 40,
        2..=3 => 60,
        4..=5 => 80,
        6..=7 => 95,
        _ => 100,
    };
    let land = match terrain(g) {
        Terrain::Mountains => 60,
        Terrain::Forest => 80,
        Terrain::Hills => 85,
        Terrain::Desert => 90,
        Terrain::Plains => 100,
    };
    let sky = match g.t.precip {
        Precip::Snow => 50,
        Precip::Rain => 85,
        Precip::Clear => 100,
    };
    let mut m = pct(pct(pct(base, oxen), land), sky);
    if g.t.ox_lame > 0 {
        m = pct(m, 75);
    }
    if g.t.health >= 105 {
        m = pct(m, 80);
    }
    let wobble = g.rng.range(85, 115) as u32;
    pct(m, wobble) / 10
}

/// One day on the move. Returns a new scene when the day ends the travel.
fn travel_day(g: &mut Game) -> Option<Scene> {
    sim_day(g, DayMode::Travel);
    let mut miles = miles_today(g);
    if g.t.precip == Precip::Snow && g.t.temp < 15 {
        g.say(ui::date(g), "Blizzard! It's too snowy to travel today.");
        miles = 0;
    } else {
        random_event(g);
    }
    if g.t.alive_count() == 0 {
        return Some(score::game_over(g));
    }
    g.t.miles += miles;
    let next = g.t.next;
    if g.data
        .landmarks
        .get(next)
        .is_some_and(|l| g.t.miles >= l.mile)
    {
        return Some(arrive_at(g, next));
    }
    None
}

pub fn arrive_at(g: &mut Game, idx: usize) -> Scene {
    let l = &g.data.landmarks[idx];
    let (name, kind, mile) = (l.name.clone(), l.kind, l.mile);
    g.t.at = Some(idx);
    g.t.next = idx + 1;
    g.t.miles = mile;
    g.tune(&[(659, 100), (784, 100), (988, 200)]);
    match kind {
        Kind::End => score::arrive(g),
        Kind::River => {
            g.say_pic(
                name.clone(),
                format!("You are now at the {name}."),
                Pic::Landmark(idx),
            );
            Scene::River(river::RiverState::new(g, idx))
        }
        Kind::Dalles => {
            g.say_pic(
                name.clone(),
                format!("You are now at {name}."),
                Pic::Landmark(idx),
            );
            Scene::Dalles
        }
        Kind::Fort | Kind::Town => {
            g.say_pic(
                name.clone(),
                format!("You are now at {name}. There's a store here."),
                Pic::Landmark(idx),
            );
            Scene::SizeUp
        }
        Kind::Landmark => {
            g.say_pic(
                name.clone(),
                format!("You are now at {name}."),
                Pic::Landmark(idx),
            );
            Scene::SizeUp
        }
    }
}

// ---------------------------------------------------------------- random events

fn random_event(g: &mut Game) {
    let gentle = gentle(g);
    if !g.rng.chance(if gentle { 70 } else { 100 }) {
        return;
    }
    let t = terrain(g);
    let month = ui::month_index(g);
    let oxen_skill = skill(g) == Skill::Oxen;
    let weights = [
        6,                              // 0 thief
        8,                              // 1 broken part
        if oxen_skill { 2 } else { 5 }, // 2 ox injured
        if gentle {
            0
        } else if oxen_skill {
            1
        } else {
            3
        }, // 3 ox dies
        3,                              // 4 ox wanders off
        5,                              // 5 someone hurt
        4,                              // 6 lost trail
        3,                              // 7 rough trail
        4,                              // 8 bad water
        if t == Terrain::Desert { 6 } else { 2 }, // 9 little water
        if matches!(t, Terrain::Desert | Terrain::Mountains) {
            4
        } else {
            1
        }, // 10 no grass
        if matches!(t, Terrain::Hills | Terrain::Forest) {
            4
        } else {
            1
        }, // 11 fog
        if t == Terrain::Plains && (4..=7).contains(&month) {
            3
        } else {
            0
        }, // 12 hail
        if (5..=8).contains(&month) && t != Terrain::Desert {
            5
        } else {
            1
        }, // 13 wild fruit
        4,                              // 14 abandoned wagon
        if gentle { 4 } else { 1 },     // 15 helpful family
        2,                              // 16 fire
        4,                              // 17 just for fun
    ];
    let Some(event) = g.rng.weighted(&weights) else {
        return;
    };
    let date = ui::date(g);
    let s = &mut g.t.sup;
    match event {
        0 => {
            let lost = match g.rng.below(3) {
                0 if s.ammo > 0 => {
                    let n = s.ammo.min(g.rng.range(20, 50) as u32);
                    s.ammo -= n;
                    format!("{n} bullets")
                }
                1 if s.clothing > 0 => {
                    let n = s.clothing.min(g.rng.range(1, 2) as u32);
                    s.clothing -= n;
                    format!("{n} sets of clothing")
                }
                _ => {
                    let n = s.food * g.rng.range(10, 25) as u32 / 100;
                    s.food -= n;
                    format!("{n} pounds of food")
                }
            };
            if gentle {
                g.say(
                    date,
                    format!(
                        "A sneaky raccoon got into the wagon at night and ran off with {lost}!"
                    ),
                );
            } else {
                g.say(
                    date,
                    format!("A thief comes during the night and steals {lost}."),
                );
            }
        }
        1 => {
            let which = g.rng.below(3);
            let (name, spare) = match which {
                0 => ("wheel", &mut s.wheels),
                1 => ("axle", &mut s.axles),
                _ => ("tongue", &mut s.tongues),
            };
            if *spare > 0 {
                *spare -= 1;
                g.say(
                    date,
                    format!("A wagon {name} breaks. You replace it with your spare."),
                );
            } else {
                let fix = if skill(g) == Skill::Repair { 800 } else { 500 };
                if g.rng.chance(fix) {
                    g.say(
                        date,
                        format!("A wagon {name} breaks. You were able to repair it. Lose 1 day."),
                    );
                    pass_days(g, 1, DayMode::Busy);
                } else {
                    g.say(
                        date,
                        format!("A wagon {name} breaks and can't be fixed. It takes 3 days to carve a new one from a tree."),
                    );
                    pass_days(g, 3, DayMode::Busy);
                }
            }
            g.beep(150, 150);
        }
        2 if s.oxen > 0 => {
            g.t.ox_lame = 10;
            g.say(
                date,
                "One of your oxen is injured. Your wagon will be slower for a while.",
            );
        }
        3 if s.oxen > 0 => {
            s.oxen -= 1;
            g.say(date, "One of your oxen has died.");
        }
        4 if s.oxen > 0 => {
            g.say(
                date,
                "One of your oxen wandered off. You spend a day finding it.",
            );
            pass_days(g, 1, DayMode::Busy);
        }
        5 => {
            if let Some(i) = someone(g).filter(|&i| g.t.party[i].sick.is_none()) {
                let hurt = if gentle {
                    ["a sprained ankle", "a bee sting", "a bumped head"][g.rng.below(3) as usize]
                } else {
                    ["a snakebite", "a broken arm", "a broken leg"][g.rng.below(3) as usize]
                };
                let name = g.t.party[i].name.clone();
                g.t.party[i].sick = Some((String::from(hurt), 0));
                g.say(date, format!("{name} has {hurt}."));
                g.beep(220, 120);
            }
        }
        6 => {
            let n = g.rng.range(1, 4) as u32;
            let what = if gentle {
                "You took a wrong turn"
            } else {
                "Lost trail"
            };
            g.say(date, format!("{what}. Lose {n} days."));
            pass_days(g, n, DayMode::Busy);
        }
        7 => {
            g.say(date, "Rough trail. Lose 1 day.");
            pass_days(g, 1, DayMode::Busy);
        }
        8 => {
            g.t.health += 15;
            let text = if gentle {
                "The water tastes yucky. Everyone feels a little sick."
            } else {
                "Bad water."
            };
            g.say(date, text);
        }
        9 => {
            g.t.health += 10;
            g.say(date, "Very little water. Everyone is thirsty.");
        }
        10 => {
            g.t.ox_lame = g.t.ox_lame.max(5);
            g.say(
                date,
                "Not enough grass for the oxen. They are getting weak.",
            );
        }
        11 => {
            g.say(date, "Heavy fog. Lose 1 day.");
            pass_days(g, 1, DayMode::Busy);
        }
        12 => {
            let n = s.food / 10;
            s.food -= n;
            g.say(
                date,
                format!("A huge hailstorm! It ruins {n} pounds of food."),
            );
        }
        13 => {
            let n = g.rng.range(20, 60) as u32;
            s.food += n;
            let fruit = ["wild berries", "wild plums", "chokecherries"][g.rng.below(3) as usize];
            g.say(date, format!("You find {fruit}! You gather {n} pounds."));
            g.tune(&[(784, 60), (988, 60), (1175, 90)]);
        }
        14 => {
            let food = g.rng.range(20, 80) as u32;
            let ammo = g.rng.range(0, 3) as u32 * 20;
            let clothes = g.rng.range(0, 2) as u32;
            s.food += food;
            s.ammo += ammo;
            s.clothing += clothes;
            let mut found = format!("{food} pounds of food");
            if ammo > 0 {
                found.push_str(&format!(", {ammo} bullets"));
            }
            if clothes > 0 {
                found.push_str(&format!(", {clothes} sets of clothing"));
            }
            g.say(
                date,
                format!("You find an abandoned wagon. Inside: {found}."),
            );
        }
        15 => {
            s.food += 50;
            g.say(date, "A friendly family shares 50 pounds of food with you.");
        }
        16 => {
            let n = s.food * g.rng.range(10, 30) as u32 / 100;
            s.food -= n;
            if gentle {
                g.say(
                    date,
                    format!("The campfire got too big and burned {n} pounds of food. Oops!"),
                );
            } else {
                g.say(
                    date,
                    format!("A fire in the wagon destroys {n} pounds of food."),
                );
            }
        }
        17 => {
            let who = someone(g).map_or(String::from("Someone"), |i| g.t.party[i].name.clone());
            let fun = [
                format!("{who} found a really cool rock."),
                String::from("A rainbow stretches across the whole sky."),
                String::from("A prairie dog pops up and stares at you."),
                String::from("Shooting stars light up the night sky."),
                format!("{who} learned to whistle with a blade of grass."),
                format!("{who} spotted an eagle soaring overhead."),
            ];
            let pick = g.rng.below(fun.len() as u32) as usize;
            g.say(date, fun[pick].clone());
        }
        _ => {}
    }
}

// ---------------------------------------------------------------- the travel screen

fn sky(g: &Game) -> gfx::Color {
    match g.t.precip {
        Precip::Clear if g.t.temp < 30 => gfx::rgb(170, 190, 210),
        Precip::Clear => gfx::SKY,
        Precip::Rain => gfx::rgb(110, 120, 140),
        Precip::Snow => gfx::rgb(190, 195, 205),
    }
}

pub fn render_travel(g: &Game, f: &mut Frame, acc_ms: u32) {
    let scroll = (g.t.miles * 6) as i32 + (acc_ms / 60) as i32;
    pictures::backdrop(f, terrain(g), 0, 72, scroll, sky(g));
    let horizon = 72 * 3 / 5;
    if let Some(l) = g.data.landmarks.get(g.t.next) {
        let dist = l.mile.saturating_sub(g.t.miles) as i32;
        if dist < 40 {
            pictures::mini_landmark(f, &l.picture, 90 - dist * 3, horizon);
        }
    }
    pictures::team(f, 110, 46, (g.clock / 250) as usize);
    match g.t.precip {
        Precip::Rain => {
            for i in 0..40 {
                let x = (i * 47 + (g.clock / 20) as i32) % W;
                let y = (i * 23 + (g.clock / 10) as i32) % 72;
                f.rect(x, y, 1, 3, gfx::rgb(200, 210, 255));
            }
        }
        Precip::Snow => {
            for i in 0..40 {
                let x = (i * 47 + (g.clock / 60) as i32) % W;
                let y = (i * 23 + (g.clock / 40) as i32) % 72;
                f.rect(x, y, 2, 2, gfx::WHITE);
            }
        }
        Precip::Clear => {}
    }
    f.label(
        3,
        2,
        "ENTER: size up the situation",
        gfx::WHITE,
        gfx::BLACK,
        Font::Small,
    );

    let next = g
        .data
        .landmarks
        .get(g.t.next)
        .map_or(0, |l| l.mile.saturating_sub(g.t.miles));
    let lines = [
        ("Date:", ui::date(g)),
        ("Weather:", String::from(ui::weather_word(g))),
        ("Health:", String::from(ui::health_word(g.t.health))),
        ("Food:", format!("{} pounds", g.t.sup.food)),
        ("Next landmark:", format!("{next} miles")),
        ("Miles traveled:", format!("{} miles", g.t.miles)),
    ];
    let mut y = 75;
    for (k, v) in lines {
        f.text(40, y, k, gfx::GRAY, Font::Normal);
        f.text(136, y, &v, gfx::WHITE, Font::Normal);
        y += 10;
    }
}

pub fn travel_tick(g: &mut Game, mut acc: u32, ms: u32) -> Scene {
    if g.t.sup.oxen == 0 {
        g.say(
            "No oxen",
            "You have no oxen to pull your wagon! Rest and hope for help, or trade for oxen.",
        );
        return Scene::SizeUp;
    }
    acc += ms;
    let day_ms = (DAY_MS / g.cheats.speed.max(1)).max(20);
    while acc >= day_ms {
        acc -= day_ms;
        if let Some(scene) = travel_day(g) {
            return scene;
        }
        if !g.msgs.is_empty() {
            break;
        }
    }
    Scene::Travel { acc_ms: acc }
}

pub fn travel_key(_g: &mut Game, acc_ms: u32, k: Key) -> Scene {
    if matches!(k, Key::Enter | Key::Escape | Key::Char(' ')) {
        Scene::SizeUp
    } else {
        Scene::Travel { acc_ms }
    }
}

// ---------------------------------------------------------------- size up the situation

#[derive(Clone, Copy, PartialEq, Eq)]
enum Act {
    Continue,
    Supplies,
    Map,
    Pace,
    Rations,
    Rest,
    Trade,
    Talk,
    Buy,
}

fn here(g: &Game) -> Option<&crate::data::Landmark> {
    g.t.at.and_then(|i| g.data.landmarks.get(i))
}

fn actions(g: &Game) -> Vec<(Act, &'static str)> {
    let mut v = alloc::vec![
        (Act::Continue, "Continue on trail"),
        (Act::Supplies, "Check supplies"),
        (Act::Map, "Look at map"),
        (Act::Pace, "Change pace"),
        (Act::Rations, "Change rations"),
        (Act::Rest, "Stop to rest"),
        (Act::Trade, "Attempt to trade"),
    ];
    if let Some(l) = here(g) {
        if !l.talk.is_empty() {
            v.push((Act::Talk, "Talk to people"));
        }
        if matches!(l.kind, Kind::Fort | Kind::Town) {
            v.push((Act::Buy, "Buy supplies"));
        }
    }
    v
}

fn can_fish(g: &Game) -> bool {
    here(g).is_some_and(|l| {
        matches!(l.kind, Kind::River | Kind::Dalles | Kind::Fort | Kind::Town)
            || l.picture == "springs"
    })
}

pub fn render_sizeup(g: &Game, f: &mut Frame) {
    let title = here(g).map_or_else(|| String::from("On the trail"), |l| l.name.clone());
    ui::header(f, &title);
    let rows = [
        format!("{}   Weather: {}", ui::date(g), ui::weather_word(g)),
        format!(
            "Health: {}   Pace: {}",
            ui::health_word(g.t.health),
            ui::pace_word(g.t.pace)
        ),
        format!(
            "Rations: {}   Food: {} lbs",
            ui::rations_word(g.t.rations),
            g.t.sup.food
        ),
    ];
    for (i, r) in rows.iter().enumerate() {
        f.text(3, 14 + i as i32 * 10, r, gfx::GRAY, Font::Normal);
    }
    let acts = actions(g);
    let mut items: Vec<String> = acts
        .iter()
        .enumerate()
        .map(|(i, (_, s))| format!("{}. {s}", i + 1))
        .collect();
    items.push(String::from("H. Go hunting"));
    if can_fish(g) {
        items.push(String::from("F. Go fishing"));
    }
    let per_col = items.len().div_ceil(2);
    for (i, s) in items.iter().enumerate() {
        let x = if i < per_col { 2 } else { 126 };
        let y = 48 + (i % per_col) as i32 * 10;
        f.text(x, y, s, gfx::WHITE, Font::Normal);
    }
    if g.cheats.peek {
        f.text(
            3,
            116,
            &format!(
                "[peek] hp={} lame={} temp={}F oxen={}",
                g.t.health, g.t.ox_lame, g.t.temp, g.t.sup.oxen
            ),
            gfx::GREEN,
            Font::Small,
        );
    }
    ui::footer(f, "What is your choice?");
}

pub fn sizeup_key(g: &mut Game, k: Key) -> Scene {
    match k {
        Key::Char('h' | 'H') => {
            if g.t.sup.ammo == 0 && !g.cheats.infinite_ammo {
                g.say(
                    "Hunting",
                    "You don't have any bullets. Buy ammunition at a store.",
                );
                return Scene::SizeUp;
            }
            return Scene::Hunt(alloc::boxed::Box::new(hunt::HuntState::new(g)));
        }
        Key::Char('f' | 'F') if can_fish(g) => {
            return Scene::Fish(alloc::boxed::Box::default());
        }
        _ => {}
    }
    let acts = actions(g);
    let Some(i) = ui::digit(k).filter(|i| *i <= acts.len()) else {
        return Scene::SizeUp;
    };
    g.beep(587, 25);
    match acts[i - 1].0 {
        Act::Continue => {
            if g.t.sup.oxen == 0 {
                g.say(
                    "No oxen",
                    "You have no oxen to pull your wagon! Try resting or trading.",
                );
                return Scene::SizeUp;
            }
            if here(g).is_some_and(|l| l.kind == Kind::Dalles) {
                return Scene::Dalles;
            }
            g.t.at = None;
            Scene::Travel { acc_ms: 0 }
        }
        Act::Supplies => Scene::Supplies,
        Act::Map => Scene::Map,
        Act::Pace => Scene::Pace,
        Act::Rations => Scene::Rations,
        Act::Rest => Scene::Rest { buf: String::new() },
        Act::Trade => Scene::Trade(make_offer(g)),
        Act::Talk => {
            let Some(i) = g.t.at else {
                return Scene::SizeUp;
            };
            let pick = g.rng.below(g.data.landmarks[i].talk.len() as u32) as usize;
            let l = &g.data.landmarks[i];
            let (name, line) = (l.name.clone(), l.talk[pick].clone());
            g.say(name, format!("A traveler tells you:\n\"{line}\""));
            Scene::SizeUp
        }
        Act::Buy => Scene::Store(store::StoreState::new(true)),
    }
}

// ---------------------------------------------------------------- supplies & map

pub fn render_supplies(g: &Game, f: &mut Frame) {
    ui::header(f, "Your supplies");
    let s = &g.t.sup;
    let rows = [
        ("Oxen", format!("{}", s.oxen)),
        ("Sets of clothing", format!("{}", s.clothing)),
        ("Bullets", format!("{}", s.ammo)),
        ("Wagon wheels", format!("{}", s.wheels)),
        ("Wagon axles", format!("{}", s.axles)),
        ("Wagon tongues", format!("{}", s.tongues)),
        ("Pounds of food", format!("{}", s.food)),
        ("Money left", ui::money(s.cash)),
    ];
    let mut y = 15;
    for (k, v) in rows {
        f.text(12, y, k, gfx::GRAY, Font::Normal);
        f.text(150, y, &v, gfx::WHITE, Font::Normal);
        y += 10;
    }
    let mut x = 3;
    for p in &g.t.party {
        let c = if p.gone {
            gfx::DARK_GRAY
        } else if p.sick.is_some() {
            gfx::ORANGE
        } else {
            gfx::GREEN
        };
        x += f.text(x, y + 2, &p.name, c, Font::Small) + 6;
    }
    ui::footer(f, "Press any key. (orange = sick)");
}

fn map_point(g: &Game, mile: u32, i: i32) -> Point {
    let x = 10 + (mile * 220 / g.data.trail_length()) as i32;
    let y = 74 + ((i * 5) % 7 - 3) * 3 - (mile as i32 * 12 / g.data.trail_length() as i32);
    Point::new(x, y)
}

pub fn render_map(g: &Game, f: &mut Frame) {
    ui::header(f, "Map of the trail");
    f.rect(0, 12, W, 113, gfx::rgb(200, 180, 130));
    for (cx, w) in [(118, 30), (135, 24), (195, 20)] {
        pictures::mountain(f, cx, 40, 62, w, gfx::rgb(150, 130, 100), false);
    }
    let lms = &g.data.landmarks;
    for i in 1..lms.len() {
        let a = map_point(g, lms[i - 1].mile, i as i32 - 1);
        let b = map_point(g, lms[i].mile, i as i32);
        let done = lms[i].mile <= g.t.miles;
        let c = if done { gfx::RED } else { gfx::DARK_BROWN };
        let _ = Line::new(a, b)
            .into_styled(PrimitiveStyle::with_stroke(c, 1))
            .draw(f);
    }
    for (i, l) in lms.iter().enumerate() {
        let p = map_point(g, l.mile, i as i32);
        let c = match l.kind {
            Kind::River => gfx::BLUE,
            Kind::Fort | Kind::Town => gfx::DARK_BROWN,
            _ => gfx::BLACK,
        };
        f.rect(p.x - 1, p.y - 1, 3, 3, c);
    }
    // Where the wagon is now, between the last landmark and the next.
    let prev = g.t.next.saturating_sub(1).min(lms.len() - 1);
    let next = g.t.next.min(lms.len() - 1);
    let a = map_point(g, lms[prev].mile, prev as i32);
    let b = map_point(g, lms[next].mile, next as i32);
    let span = lms[next].mile.saturating_sub(lms[prev].mile).max(1) as i32;
    let done = g.t.miles.saturating_sub(lms[prev].mile).min(span as u32) as i32;
    let here = Point::new(
        a.x + (b.x - a.x) * done / span,
        a.y + (b.y - a.y) * done / span,
    );
    if (g.clock / 300) % 2 == 0 {
        f.rect(here.x - 2, here.y - 2, 5, 5, gfx::RED);
    }
    f.text(
        4,
        108,
        lms[0].name.split(',').next().unwrap_or(""),
        gfx::BLACK,
        Font::Small,
    );
    f.text(196, 108, "Oregon", gfx::BLACK, Font::Small);
    let next_name = lms.get(g.t.next).map_or("", |l| l.name.as_str());
    f.text(
        4,
        116,
        &format!("Next: {next_name}"),
        gfx::DARK_BLUE,
        Font::Small,
    );
    ui::footer(f, "Press any key.");
}

// ---------------------------------------------------------------- pace, rations, rest

pub fn render_pace(g: &Game, f: &mut Frame) {
    ui::header(f, &format!("Pace: {}", ui::pace_word(g.t.pace)));
    let y = ui::menu(f, 3, 16, &["Steady", "Strenuous", "Grueling"]);
    ui::para(
        f,
        3,
        y + 4,
        "Steady: 8 hours a day, rest often. Strenuous: 12 hours, people get tired. Grueling: 16 hours, people get sick fast!",
        gfx::GRAY,
    );
}

pub fn pace_key(g: &mut Game, k: Key) -> Scene {
    if let Some(i @ 1..=3) = ui::digit(k) {
        g.t.pace = i - 1;
    }
    Scene::SizeUp
}

pub fn render_rations(g: &Game, f: &mut Frame) {
    ui::header(f, &format!("Rations: {}", ui::rations_word(g.t.rations)));
    let y = ui::menu(f, 3, 16, &["Filling", "Meager", "Bare bones"]);
    ui::para(
        f,
        3,
        y + 4,
        "Filling: big meals (3 lbs each a day). Meager: small meals (2 lbs). Bare bones: barely enough (1 lb). Hungry people get sick.",
        gfx::GRAY,
    );
}

pub fn rations_key(g: &mut Game, k: Key) -> Scene {
    if let Some(i @ 1..=3) = ui::digit(k) {
        g.t.rations = i - 1;
    }
    Scene::SizeUp
}

pub fn render_rest(g: &Game, f: &mut Frame, buf: &str) {
    ui::header(f, "Stop to rest");
    ui::para(
        f,
        3,
        18,
        "Resting helps sick people and tired oxen get better. You still eat food.",
        gfx::GRAY,
    );
    let cursor = if (g.clock / 400) % 2 == 0 { "_" } else { " " };
    f.text(
        3,
        60,
        &format!("How many days? (1-9) {buf}{cursor}"),
        gfx::YELLOW,
        Font::Normal,
    );
    ui::footer(f, "Type a number, ENTER.  ESC: cancel.");
}

pub fn rest_key(g: &mut Game, mut buf: String, k: Key) -> Scene {
    match k {
        Key::Char(c @ '1'..='9') => {
            buf.clear();
            buf.push(c);
        }
        Key::Backspace => buf.clear(),
        Key::Escape => return Scene::SizeUp,
        Key::Enter => {
            let days: u32 = buf.parse().unwrap_or(0);
            if days == 0 {
                return Scene::SizeUp;
            }
            for _ in 0..days {
                sim_day(g, DayMode::Rest);
                if g.t.sup.oxen == 0 && g.rng.chance(150) {
                    g.t.sup.oxen = 2;
                    g.say(
                        ui::date(g),
                        "A passing family has two extra oxen and gives them to you!",
                    );
                }
            }
            if g.t.alive_count() == 0 {
                return score::game_over(g);
            }
            g.say("Rest", format!("You rested for {days} days."));
            return Scene::SizeUp;
        }
        _ => {}
    }
    Scene::Rest { buf }
}

// ---------------------------------------------------------------- trading

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Oxen,
    Food,
    Clothing,
    Ammo,
    Wheel,
    Axle,
    Tongue,
}

pub struct Offer {
    pub give: (Item, u32),
    pub get: (Item, u32),
}

fn item_text(item: Item, n: u32) -> String {
    match item {
        Item::Oxen => format!("{n} oxen"),
        Item::Food => format!("{n} pounds of food"),
        Item::Clothing => format!("{n} sets of clothing"),
        Item::Ammo => format!("{n} bullets"),
        Item::Wheel => format!("{n} wagon wheel"),
        Item::Axle => format!("{n} wagon axle"),
        Item::Tongue => format!("{n} wagon tongue"),
    }
}

fn amount(g: &mut Game, item: Item) -> u32 {
    match item {
        Item::Oxen => g.rng.range(1, 2) as u32,
        Item::Food => g.rng.range(5, 20) as u32 * 10,
        Item::Clothing => g.rng.range(1, 3) as u32,
        Item::Ammo => g.rng.range(1, 5) as u32 * 20,
        _ => 1,
    }
}

fn stock(g: &mut Game, item: Item) -> &mut u32 {
    let s = &mut g.t.sup;
    match item {
        Item::Oxen => &mut s.oxen,
        Item::Food => &mut s.food,
        Item::Clothing => &mut s.clothing,
        Item::Ammo => &mut s.ammo,
        Item::Wheel => &mut s.wheels,
        Item::Axle => &mut s.axles,
        Item::Tongue => &mut s.tongues,
    }
}

const ITEMS: [Item; 7] = [
    Item::Oxen,
    Item::Food,
    Item::Clothing,
    Item::Ammo,
    Item::Wheel,
    Item::Axle,
    Item::Tongue,
];

fn make_offer(g: &mut Game) -> Option<Offer> {
    if g.rng.chance(250) {
        return None;
    }
    let a = ITEMS[g.rng.below(7) as usize];
    let mut b = ITEMS[g.rng.below(7) as usize];
    while b == a {
        b = ITEMS[g.rng.below(7) as usize];
    }
    let give = (a, amount(g, a));
    let get = (b, amount(g, b));
    Some(Offer { give, get })
}

pub fn render_trade(g: &Game, f: &mut Frame, offer: Option<&Offer>) {
    ui::header(f, "Trading");
    match offer {
        None => {
            ui::para(
                f,
                3,
                18,
                "Nobody wants to trade with you today.",
                gfx::WHITE,
            );
            ui::footer(f, "Press any key.");
        }
        Some(o) => {
            let text = format!(
                "A traveler would like {}. In return you'll get {}.",
                item_text(o.give.0, o.give.1),
                item_text(o.get.0, o.get.1)
            );
            let y = ui::para(f, 3, 18, &text, gfx::WHITE);
            let have = match o.give.0 {
                Item::Oxen => g.t.sup.oxen,
                Item::Food => g.t.sup.food,
                Item::Clothing => g.t.sup.clothing,
                Item::Ammo => g.t.sup.ammo,
                Item::Wheel => g.t.sup.wheels,
                Item::Axle => g.t.sup.axles,
                Item::Tongue => g.t.sup.tongues,
            };
            f.text(
                3,
                y + 6,
                &format!("You have {}.", item_text(o.give.0, have)),
                gfx::GRAY,
                Font::Normal,
            );
            f.text(
                3,
                y + 22,
                "Are you willing to trade? (Y/N)",
                gfx::YELLOW,
                Font::Normal,
            );
        }
    }
}

pub fn trade_key(g: &mut Game, offer: Option<Offer>, k: Key) -> Scene {
    let Some(o) = offer else {
        sim_day(g, DayMode::Busy);
        return Scene::SizeUp;
    };
    if ui::yes(k) {
        if *stock(g, o.give.0) < o.give.1 {
            g.say("Trading", "You don't have enough to make that trade.");
        } else {
            *stock(g, o.give.0) -= o.give.1;
            let got = stock(g, o.get.0);
            *got = got.saturating_add(o.get.1);
            g.say("Trading", "It's a deal!");
            g.tune(&[(784, 40), (1047, 60)]);
        }
    } else if !ui::no(k) {
        return Scene::Trade(Some(o));
    }
    sim_day(g, DayMode::Busy);
    if g.t.alive_count() == 0 {
        return score::game_over(g);
    }
    Scene::SizeUp
}
