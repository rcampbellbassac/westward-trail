//! Reaching Oregon, points, the Top Ten, and game over.

use alloc::{format, string::String, vec::Vec};

use crate::{
    Game, Mood, Pic, Scene, ScoreEntry,
    gfx::{self, Font, Frame},
    pictures, ui,
};

pub fn default_top_ten() -> Vec<ScoreEntry> {
    [
        ("Stephen Meek", 7650),
        ("Celinda Hines", 5694),
        ("Andrew Sublette", 4138),
        ("David Hastings", 2945),
        ("Ezra Meeker", 2052),
        ("Willam Vaughn", 1401),
        ("Mary Bartlett", 1033),
        ("William Wiggins", 966),
        ("Charles Hopper", 512),
        ("Elijah White", 284),
    ]
    .into_iter()
    .map(|(n, p)| ScoreEntry {
        name: String::from(n),
        points: p,
    })
    .collect()
}

pub fn export(top: &[ScoreEntry]) -> String {
    top.iter()
        .map(|e| format!("{}|{}\n", e.name, e.points))
        .collect()
}

pub fn import(s: &str) -> Option<Vec<ScoreEntry>> {
    let top: Vec<ScoreEntry> = s
        .lines()
        .filter_map(|l| {
            let (n, p) = l.rsplit_once('|')?;
            Some(ScoreEntry {
                name: String::from(n.trim()),
                points: p.trim().parse().ok()?,
            })
        })
        .take(10)
        .collect();
    (!top.is_empty()).then_some(top)
}

fn rating(points: u32) -> &'static str {
    match points {
        3000.. => "Trail guide",
        1500.. => "Adventurer",
        _ => "Greenhorn",
    }
}

pub fn arrive(g: &mut Game) -> Scene {
    let end = g.data.landmarks.len() - 1;
    g.say_pic(
        g.data.landmarks[end].name.clone(),
        "Congratulations! You made it to Oregon! Let's see how many points you earned.",
        Pic::Landmark(end),
    );
    g.tune(&[
        (523, 150),
        (659, 150),
        (784, 150),
        (1047, 300),
        (784, 150),
        (1047, 500),
    ]);

    let s = &g.t.sup;
    let points_for_health = match g.t.health {
        0..35 => 500,
        35..70 => 400,
        70..105 => 300,
        _ => 200,
    };
    let people = g.t.alive_count();
    let mut lines: Vec<(String, u32)> = alloc::vec![
        (
            format!("{people} people in {} health", ui::health_word(g.t.health)),
            people * points_for_health
        ),
        (String::from("1 wagon"), 50),
        (format!("{} oxen", s.oxen), s.oxen * 4),
        (
            format!("{} spare wagon parts", s.wheels + s.axles + s.tongues),
            (s.wheels + s.axles + s.tongues) * 2
        ),
        (format!("{} sets of clothing", s.clothing), s.clothing * 2),
        (format!("{} bullets", s.ammo), s.ammo / 50),
        (format!("{} pounds of food", s.food), s.food / 25),
        (format!("{} cash", ui::money(s.cash)), s.cash / 500),
    ];
    let subtotal: u32 = lines.iter().map(|(_, p)| p).sum();
    let bonus = g
        .data
        .occupations
        .get(g.t.occupation)
        .map_or(1, |o| o.bonus);
    let total = subtotal.saturating_mul(bonus);
    lines.push((
        format!("Bonus for being a {}: x{bonus}", occupation_short(g)),
        0,
    ));
    lines.push((String::from("TOTAL"), total));

    let mut name =
        g.t.party
            .first()
            .map_or(String::from("Pioneer"), |p| p.name.clone());
    if g.cheats.used {
        name.push_str(" [H]");
    }
    let pos = g
        .top
        .iter()
        .position(|e| total > e.points)
        .unwrap_or(g.top.len());
    if pos < 10 {
        g.top.insert(
            pos,
            ScoreEntry {
                name,
                points: total,
            },
        );
        g.top.truncate(10);
    }
    Scene::Score(lines)
}

fn occupation_short(g: &Game) -> String {
    let name = g
        .data
        .occupations
        .get(g.t.occupation)
        .map_or("pioneer", |o| o.name.as_str());
    String::from(name.split(" from ").next().unwrap_or(name)).to_lowercase()
}

pub fn render_score(g: &Game, f: &mut Frame, lines: &[(String, u32)]) {
    ui::header(f, "Points for arriving in Oregon");
    let mut y = 15;
    for (label, pts) in lines {
        let total = label == "TOTAL";
        let c = if total { gfx::YELLOW } else { gfx::WHITE };
        f.text(3, y, label, c, Font::Normal);
        if *pts > 0 || total {
            f.text(196, y, &format!("{pts:>6}"), c, Font::Normal);
        }
        y += 10;
    }
    if let Some((_, total)) = lines.last() {
        f.text(
            3,
            y + 2,
            &format!("Rating: {}", rating(*total)),
            gfx::GREEN,
            Font::Normal,
        );
    }
    if g.cheats.used {
        f.text(150, y + 2, "HACKER MODE", gfx::GREEN, Font::Small);
    }
    ui::footer(f, "Press any key to see the Top Ten.");
}

pub fn after_score(_g: &mut Game) -> Scene {
    Scene::TopTen
}

pub fn render_top_ten(g: &Game, f: &mut Frame) {
    ui::header(f, "The Westward Top Ten");
    let mut y = 15;
    for (i, e) in g.top.iter().enumerate() {
        f.text(
            3,
            y,
            &format!("{:>2}. {}", i + 1, e.name),
            gfx::WHITE,
            Font::Normal,
        );
        f.text(
            150,
            y,
            &format!("{:>5}", e.points),
            gfx::YELLOW,
            Font::Normal,
        );
        f.text(186, y, rating(e.points), gfx::GRAY, Font::Small);
        y += 10;
    }
    ui::footer(f, "Press any key.");
}

pub fn game_over(g: &mut Game) -> Scene {
    if g.t.mood == Mood::Gentle {
        g.say(
            "The trail is over",
            "Everyone has gone home to rest. Maybe next time you'll make it all the way to Oregon!",
        );
    } else {
        g.say(
            "The trail is over",
            "Everyone in your party has died. The trail to Oregon is a hard one.",
        );
    }
    Scene::GameOver
}

pub fn render_game_over(g: &Game, f: &mut Frame) {
    ui::header(f, "Game over");
    if g.t.mood == Mood::Gentle {
        pictures::backdrop(f, crate::data::Terrain::Plains, 12, 70, 0, gfx::SKY);
        pictures::team(f, 100, 50, 0);
    } else {
        let leader = g.t.party.first().map_or("", |p| p.name.as_str());
        pictures::tomb(f, leader, 12, 70);
    }
    f.text_center(
        92,
        "Try again? The trail is waiting.",
        gfx::WHITE,
        Font::Normal,
    );
    ui::footer(f, "Press any key.");
}
