//! Title screen and getting ready: tone, occupation, names, month.

use alloc::{format, string::String, vec::Vec};

use crate::{
    Game, Key, Mood, Person, Pic, Scene, Trip,
    gfx::{self, Font, Frame, W},
    pictures, store, ui,
};

/// The months you can leave in, as on the month menu.
pub const MONTH_NAMES: [&str; 5] = ["March", "April", "May", "June", "July"];

const LEARN: [&str; 7] = [
    "In the 1840s, thousands of families packed everything they owned into covered wagons and set out from Independence, Missouri for Oregon's Willamette Valley, about 2,000 miles away.",
    "You'll buy supplies, cross rivers, hunt for food, and decide how fast to travel and how much to eat. Push too hard and your party gets sick. Go too slow and winter catches you in the mountains.",
    "Leave too early and there's no grass for the oxen. Leave too late and you'll meet snow. Keep an eye on your food, your clothing, and everyone's health.",
    "Pioneers also gathered wild food: berries, roots and mushrooms. What grows changes as you travel west and as the months go by. Press G on the trail. In real life, never eat a wild plant unless a grown-up who knows plants says it's safe!",
    "Press S on the trail to save your trip in one of five slots, and pick \"Load a saved trip\" on the title screen to keep going later. On the Cardputer, Fn+P saves a screenshot to the SD card.",
    "Controls: number keys pick menu choices, ENTER continues. Hunting and rafting: arrow keys or W A S D move (on the Cardputer, ; , . / work too). SPACE shoots or reels in.",
    "Psst... real pioneers fixed their own wagons. Real hackers fix their own games. Press the ` key (top left) and type  help",
];

pub fn render_title(g: &Game, f: &mut Frame) {
    f.clear(gfx::rgb(10, 20, 50));
    for i in 0..30 {
        let x = (i * 53 + 7) % W;
        let y = (i * 29 + 3) % 50;
        let twinkle = (g.clock / 300 + i as u32) % 7 == 0;
        f.pixel(x, y, if twinkle { gfx::YELLOW } else { gfx::WHITE });
    }
    let font = if g.data.title.len() * 10 <= 236 {
        Font::Big
    } else {
        Font::Bold
    };
    f.text_center(8, &g.data.title, gfx::YELLOW, font);
    f.rect(0, 92, W, 43, gfx::rgb(90, 150, 60));
    pictures::mountain(f, 40, 70, 92, 40, gfx::rgb(70, 70, 110), true);
    pictures::mountain(f, 200, 60, 92, 50, gfx::rgb(80, 80, 120), true);
    let x = W - ((g.clock / 40) as i32 % (W + 60));
    pictures::team(f, x, 82, (g.clock / 250) as usize);
    ui::menu(
        f,
        50,
        36,
        &[
            "Travel the trail",
            "Learn about the trail",
            "See the Top Ten",
            "Load a saved trip",
            "Play together",
        ],
    );
    f.text(3, 126, "v0.1", gfx::rgb(60, 100, 40), Font::Small);
}

/// The opening of "Oh! Susanna" (Stephen Foster, 1848: the year of our trip).
/// (Hz, ms) pairs; 0 Hz is a rest, used to split repeated notes.
const SUSANNA: &[(u16, u16)] = &[
    // I come from Al-a-bam-a with my ban-jo on my knee,
    (523, 180),
    (587, 180),
    (659, 180),
    (784, 160),
    (0, 20),
    (784, 270),
    (880, 90),
    (784, 180),
    (659, 180),
    (523, 270),
    (587, 90),
    (659, 160),
    (0, 20),
    (659, 180),
    (587, 180),
    (523, 180),
    (587, 540),
    (0, 180),
    // I'm goin' to Lou'-si-an-a, my true love for to see.
    (523, 180),
    (587, 180),
    (659, 180),
    (784, 160),
    (0, 20),
    (784, 270),
    (880, 90),
    (784, 180),
    (659, 180),
    (523, 270),
    (587, 90),
    (659, 160),
    (0, 20),
    (659, 180),
    (587, 160),
    (0, 20),
    (587, 180),
    (523, 720),
];

/// Play the title song, if the title is showing.
pub fn title_song(g: &mut Game) {
    if matches!(g.scene, Scene::Title) {
        g.tune(SUSANNA);
    }
}

pub fn title_key(g: &mut Game, k: Key) -> Scene {
    match ui::digit(k) {
        Some(1) => {
            g.beep(523, 40);
            Scene::Mood
        }
        Some(2) => Scene::Learn(0),
        Some(3) => Scene::TopTen,
        Some(4) => crate::save::open(crate::save::SlotMode::Load),
        Some(5) => crate::net::open(g),
        _ => Scene::Title,
    }
}

pub fn render_learn(_g: &Game, f: &mut Frame, page: usize) {
    ui::header(
        f,
        &format!("Learn about the trail  {}/{}", page + 1, LEARN.len()),
    );
    ui::para(f, 3, 18, LEARN[page.min(LEARN.len() - 1)], gfx::WHITE);
    ui::footer(f, "ENTER: next page   ESC: back to title");
}

pub fn learn_key(_g: &mut Game, page: usize, k: Key) -> Scene {
    if k == Key::Escape || page + 1 >= LEARN.len() {
        Scene::Title
    } else {
        Scene::Learn(page + 1)
    }
}

pub fn render_mood(_g: &Game, f: &mut Frame) {
    ui::header(f, "Who is going west today?");
    ui::menu(f, 3, 20, &["Younger pioneers", "Seasoned pioneers"]);
    ui::para(
        f,
        3,
        48,
        "Younger: gentle trail. Nobody dies; sick folks get a ride home to rest.",
        gfx::GRAY,
    );
    ui::para(
        f,
        3,
        80,
        "Seasoned: the classic trail, with all its dangers.",
        gfx::GRAY,
    );
}

pub fn mood_key(g: &mut Game, k: Key) -> Scene {
    let mood = match ui::digit(k) {
        Some(1) => Mood::Gentle,
        Some(2) => Mood::Classic,
        _ => {
            return if k == Key::Escape {
                Scene::Title
            } else {
                Scene::Mood
            };
        }
    };
    crate::net::leave(g); // a trip of your own ends any race
    g.t = Trip::new();
    g.t.mood = mood;
    g.cheats.used = false;
    Scene::Occupation
}

pub fn render_occupation(g: &Game, f: &mut Frame) {
    ui::header(f, "Choose who you are");
    ui::para(
        f,
        3,
        16,
        "Many kinds of people made the trip to Oregon. You may be a:",
        gfx::GRAY,
    );
    let mut items: Vec<&str> = g.data.occupations.iter().map(|o| o.name.as_str()).collect();
    items.push("Find out the differences");
    ui::menu(f, 3, 42, &items);
}

pub fn occupation_key(g: &mut Game, k: Key) -> Scene {
    let n = g.data.occupations.len();
    match ui::digit(k) {
        Some(i) if i <= n => {
            let o = &g.data.occupations[i - 1];
            g.t.occupation = i - 1;
            g.t.sup.cash = o.cash.saturating_mul(100);
            g.t.party.clear();
            Scene::Names {
                idx: 0,
                buf: crate::net::leader_name(g),
            }
        }
        Some(i) if i == n + 1 => {
            let mut body = String::new();
            for o in &g.data.occupations {
                body.push_str(&format!(
                    "{}: ${}. {} Score x{}.\n",
                    o.name, o.cash, o.about, o.bonus
                ));
            }
            g.say("The differences", body);
            Scene::Occupation
        }
        _ => Scene::Occupation,
    }
}

pub fn render_names(g: &Game, f: &mut Frame, idx: usize, buf: &str) {
    ui::header(f, "Your wagon party");
    let q = if idx == 0 {
        "What is the first name of the wagon leader?"
    } else {
        "What are the first names of the four other people in your party?"
    };
    let mut y = ui::para(f, 3, 16, q, gfx::GRAY) + 4;
    for (i, p) in g.t.party.iter().enumerate() {
        f.text(
            12,
            y,
            &format!("{}. {}", i + 1, p.name),
            gfx::WHITE,
            Font::Normal,
        );
        y += 10;
    }
    let cursor = if (g.clock / 400) % 2 == 0 { "_" } else { " " };
    f.text(
        12,
        y,
        &format!("{}. {}{}", idx + 1, buf, cursor),
        gfx::YELLOW,
        Font::Normal,
    );
    ui::footer(f, "Type a name, ENTER. (ENTER alone picks one.)");
}

pub fn names_key(g: &mut Game, idx: usize, mut buf: String, k: Key) -> Scene {
    match k {
        Key::Enter => {
            let name = if buf.trim().is_empty() {
                random_name(g)
            } else {
                String::from(buf.trim())
            };
            g.t.party.push(Person {
                name,
                sick: None,
                gone: false,
            });
            g.beep(587, 30);
            if idx + 1 >= 5 {
                return Scene::ConfirmNames;
            }
            return Scene::Names {
                idx: idx + 1,
                buf: String::new(),
            };
        }
        Key::Backspace => {
            buf.pop();
        }
        Key::Escape if idx > 0 => {
            g.t.party.pop();
            return Scene::Names {
                idx: idx - 1,
                buf: String::new(),
            };
        }
        Key::Char(c)
            if (c.is_alphanumeric() || c == ' ' || c == '-') && buf.chars().count() < 10 =>
        {
            buf.push(c);
        }
        _ => {}
    }
    Scene::Names { idx, buf }
}

pub fn random_name(g: &mut Game) -> String {
    let unused: Vec<&String> = g
        .data
        .names
        .iter()
        .filter(|n| !g.t.party.iter().any(|p| &p.name == *n))
        .collect();
    if unused.is_empty() {
        return format!("Pioneer {}", g.t.party.len() + 1);
    }
    let i = g.rng.below(unused.len() as u32) as usize;
    unused[i].clone()
}

pub fn render_confirm_names(g: &Game, f: &mut Frame) {
    ui::header(f, "Your wagon party");
    let mut y = 18;
    for (i, p) in g.t.party.iter().enumerate() {
        let role = if i == 0 { "  (leader)" } else { "" };
        f.text(
            12,
            y,
            &format!("{}. {}{}", i + 1, p.name, role),
            gfx::WHITE,
            Font::Normal,
        );
        y += 10;
    }
    f.text(
        3,
        y + 10,
        "Are these names correct? (Y/N)",
        gfx::YELLOW,
        Font::Normal,
    );
}

pub fn confirm_names_key(g: &mut Game, k: Key) -> Scene {
    if (ui::yes(k) || k == Key::Enter) && g.net.racing {
        // Racers all leave in the month the host picked.
        let month = char::from(b'0' + g.net.settings.month as u8);
        month_key(g, Key::Char(month))
    } else if ui::yes(k) || k == Key::Enter {
        Scene::Month
    } else if ui::no(k) {
        g.t.party.clear();
        Scene::Names {
            idx: 0,
            buf: String::new(),
        }
    } else {
        Scene::ConfirmNames
    }
}

pub fn render_month(g: &Game, f: &mut Frame) {
    ui::header(f, "When will you leave?");
    let start = &g.data.landmarks[0].name;
    let y = ui::para(
        f,
        3,
        16,
        &format!("It is {}. Choose the month to leave {start}.", g.data.year),
        gfx::GRAY,
    );
    ui::menu(
        f,
        3,
        y + 4,
        &["March", "April", "May", "June", "July", "Ask for advice"],
    );
}

pub fn month_key(g: &mut Game, k: Key) -> Scene {
    match ui::digit(k) {
        Some(i @ 1..=5) => {
            let month = i + 1; // March is index 2
            g.t.day = (0..month).map(|m| days_in(g.data.year, m)).sum::<u32>() + 1;
            let cash = ui::money(g.t.sup.cash);
            g.say_pic(
                g.data.landmarks[0].name.clone(),
                format!(
                    "Before leaving you should buy supplies. You have {cash} in cash, but you don't have to spend it all now."
                ),
                Pic::Landmark(0),
            );
            g.say(
                g.data.prices.store_name.clone(),
                "Hello! So you're going to Oregon. I can fix you up with what you need: oxen to pull your wagon, \
                 clothing for cold nights, plenty of food, ammunition for hunting, and spare wagon parts.",
            );
            Scene::Store(store::StoreState::new(false))
        }
        Some(6) => {
            g.say(
                "Advice",
                "At a meeting for folks with \"Oregon fever\" an old guide tells you: leave too early and there's no grass \
                 for the oxen. Leave too late and you'll hit snow in the mountains. April or May is usually best.",
            );
            Scene::Month
        }
        _ => Scene::Month,
    }
}

fn days_in(year: u32, month: usize) -> u32 {
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ][month % 12]
}
