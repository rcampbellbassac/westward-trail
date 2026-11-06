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
    let n = g.data.title.chars().count() as i32;
    let font = if n * Font::Big.char_w() <= W - 4 {
        Font::Big
    } else if n * Font::Normal.char_w() <= W - 4 {
        Font::Normal
    } else {
        Font::Small
    };
    f.text_center(3, &g.data.title, gfx::YELLOW, font);
    f.rect(0, 112, W, 23, gfx::rgb(90, 150, 60));
    pictures::mountain(f, 30, 70, 112, 40, gfx::rgb(70, 70, 110), true);
    pictures::mountain(f, 215, 60, 112, 50, gfx::rgb(80, 80, 120), true);
    let x = W - ((g.clock / 40) as i32 % (W + 60));
    pictures::team(f, x, 102, (g.clock / 250) as usize);
    ui::menu(
        f,
        12,
        29,
        &[
            "Travel the trail",
            "Learn the trail",
            "See the Top Ten",
            "Load a saved trip",
            "Play together",
            "Fun and games",
        ],
    );
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
        Some(6) => Scene::Games(crate::races::From::Title),
        _ => Scene::Title,
    }
}

/// The learn-about-the-trail text, cut into screens of big letters.
fn learn_pages() -> Vec<Vec<String>> {
    let mut pages = Vec::new();
    for text in LEARN {
        let lines = gfx::wrap(text, ui::COLS);
        for chunk in lines.chunks(ui::ROWS) {
            pages.push(chunk.to_vec());
        }
    }
    pages
}

pub fn render_learn(_g: &Game, f: &mut Frame, page: usize) {
    let pages = learn_pages();
    let page = page.min(pages.len() - 1);
    ui::header(f, &format!("Learn the trail {}/{}", page + 1, pages.len()));
    let mut y = ui::TOP;
    for line in &pages[page] {
        f.text(3, y, line, gfx::WHITE, Font::Normal);
        y += ui::LINE;
    }
    ui::footer(f, "ENTER: next   ESC: back");
}

pub fn learn_key(_g: &mut Game, page: usize, k: Key) -> Scene {
    if k == Key::Escape || page + 1 >= learn_pages().len() {
        Scene::Title
    } else {
        Scene::Learn(page + 1)
    }
}

pub fn render_mood(_g: &Game, f: &mut Frame) {
    ui::header(f, "Who is going west?");
    let y = ui::menu(
        f,
        3,
        ui::TOP,
        &["Younger pioneers", "Seasoned pioneers", "Survival"],
    );
    let notes = [
        ("Younger:", "nobody dies.", gfx::GREEN),
        ("Seasoned:", "the real trail.", gfx::YELLOW),
        ("Survival:", "it all goes wrong!", gfx::RED),
    ];
    let mut y = y + 6;
    for (k, v, c) in notes {
        let w = f.text(3, y, k, c, Font::Small);
        f.text(3 + w + 8, y, v, gfx::GRAY, Font::Small);
        y += 14;
    }
}

pub fn mood_key(g: &mut Game, k: Key) -> Scene {
    let mood = match ui::digit(k) {
        Some(1) => Mood::Gentle,
        Some(2) => Mood::Classic,
        Some(3) => Mood::Survival,
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
    ui::para(f, 3, ui::TOP, "You may be a:", gfx::GRAY);
    let mut items: Vec<&str> = g.data.occupations.iter().map(|o| o.name.as_str()).collect();
    items.push("What's different?");
    ui::menu(f, 3, ui::TOP + ui::LINE + 6, &items);
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
                buf: crate::net::party_name(g, 0),
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
        "Name the wagon leader:"
    } else {
        "Name the other four:"
    };
    let mut y = ui::para(f, 3, ui::TOP, q, gfx::GRAY) + 2;
    // Only the last few names fit above the one being typed.
    let shown = g.t.party.len().min(4);
    for (i, p) in g.t.party.iter().enumerate().skip(g.t.party.len() - shown) {
        f.text(
            12,
            y,
            &format!("{}. {}", i + 1, p.name),
            gfx::WHITE,
            Font::Normal,
        );
        y += ui::LINE;
    }
    let cursor = if (g.clock / 400) % 2 == 0 { "_" } else { " " };
    f.text(
        12,
        y,
        &format!("{}. {}{}", idx + 1, buf, cursor),
        gfx::YELLOW,
        Font::Normal,
    );
    ui::footer(f, "Type, ENTER. Just ENTER: pick");
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
                buf: crate::net::party_name(g, idx + 1),
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
    let mut y = ui::TOP;
    for (i, p) in g.t.party.iter().enumerate() {
        let role = if i == 0 { " (leader)" } else { "" };
        f.text(
            3,
            y,
            &format!("{}. {}{}", i + 1, p.name, role),
            gfx::WHITE,
            Font::Normal,
        );
        y += ui::LINE;
    }
    f.text(3, y + 4, "All correct? (Y/N)", gfx::YELLOW, Font::Normal);
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
    let _ = g;
    ui::menu(
        f,
        3,
        ui::TOP,
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
