//! Reaching Oregon, points, the Top Ten, and game over.

use alloc::{format, string::String, vec::Vec};

use crate::{
    Game, Mood, Pic, Scene, ScoreEntry,
    gfx::{self, Font, Frame, W},
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
    let lines: Vec<(String, u32)> = alloc::vec![
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
        (ui::sets(s.clothing), s.clothing * 2),
        (format!("{} bullets", s.ammo), s.ammo / 50),
        (format!("{} pounds of food", s.food), s.food / 25),
        (format!("{} cash", ui::money(s.cash)), s.cash / 500),
    ];
    // Only what earned points, so the list fits the screen.
    let mut lines: Vec<(String, u32)> = lines.into_iter().filter(|(_, p)| *p > 0).collect();
    let subtotal: u32 = lines.iter().map(|(_, p)| p).sum();
    let bonus = g
        .data
        .occupations
        .get(g.t.occupation)
        .map_or(1, |o| o.bonus);
    let hard = g.t.mood.rules().score;
    let total = subtotal.saturating_mul(bonus).saturating_mul(hard);
    crate::net::finished(g, total);
    let job = occupation_short(g);
    let mut job_chars = job.chars();
    let job: String = job_chars
        .next()
        .map(|c| c.to_ascii_uppercase())
        .into_iter()
        .chain(job_chars)
        .collect();
    let bonus_line = if hard > 1 {
        format!("{job} x{bonus}, survival x{hard}")
    } else {
        format!("{job} bonus: x{bonus}")
    };
    lines.push((bonus_line, 0));
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
        g.saves.push(crate::SaveOp::Scores(export(&g.top)));
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
    // The total goes in the title bar and the rating in the footer, so a long list fits.
    let total = lines
        .iter()
        .find(|(l, _)| l == "TOTAL")
        .map_or(0, |(_, p)| *p);
    ui::header(f, &format!("Points: {total}"));
    let mut y = ui::HEAD_H + 2;
    for (label, pts) in lines.iter().filter(|(l, _)| l != "TOTAL") {
        let label: String = label.chars().take(23).collect();
        f.text(3, y, &label, gfx::WHITE, Font::Small);
        if *pts > 0 {
            let n = format!("{pts}");
            let x = W - 3 - n.len() as i32 * Font::Small.char_w();
            f.text(x, y, &n, gfx::YELLOW, Font::Small);
        }
        y += 12;
    }
    if g.cheats.used && y + 13 <= ui::FOOTER_Y {
        f.text(3, y, "HACKER MODE", gfx::GREEN, Font::Small);
    }
    let rate = rating(total);
    f.text(
        3,
        ui::FOOTER_Y,
        &format!("{rate}!"),
        gfx::GREEN,
        Font::Small,
    );
    let w = (rate.len() as i32 + 2) * Font::Small.char_w();
    f.text(
        3 + w,
        ui::FOOTER_Y,
        "Any key: Top Ten",
        gfx::GRAY,
        Font::Small,
    );
}

pub fn after_score(g: &mut Game, lines: &[(String, u32)]) -> Scene {
    if crate::party::hosting(g) {
        let total = lines.last().map_or(0, |(_, p)| *p);
        crate::party::end(
            g,
            &alloc::format!("The party made it to Oregon with {total} points! Hooray!"),
        );
        crate::net::leave(g);
        return Scene::TopTen;
    }
    if g.net.racing {
        return Scene::Online(crate::net::Screen::Standings { last: true });
    }
    Scene::TopTen
}

pub fn after_game_over(g: &mut Game) -> Scene {
    if crate::party::hosting(g) {
        crate::party::end(g, "The party's trip is over. Maybe next time!");
        crate::net::leave(g);
    }
    Scene::Title
}

/// The Top Ten, five at a time: `more` shows 6 to 10.
pub fn render_top_ten(g: &Game, f: &mut Frame, more: bool) {
    ui::header(
        f,
        if more {
            "Top Ten: 6 to 10"
        } else {
            "The Top Ten"
        },
    );
    let mut y = ui::TOP;
    let skip = if more { 5 } else { 0 };
    for (i, e) in g.top.iter().enumerate().skip(skip).take(5) {
        let name: String = e.name.chars().take(14).collect();
        f.text(
            3,
            y,
            &format!("{:>2}. {}", i + 1, name),
            gfx::WHITE,
            Font::Normal,
        );
        let pts = format!("{}", e.points);
        f.text(
            W - 3 - pts.len() as i32 * 10,
            y,
            &pts,
            gfx::YELLOW,
            Font::Normal,
        );
        y += 19;
    }
    ui::footer(
        f,
        if more {
            "Any key: done"
        } else {
            "Any key: 6 to 10"
        },
    );
}

pub fn game_over(g: &mut Game) -> Scene {
    crate::net::wagon_lost(g);
    if g.t.mood == Mood::Gentle {
        g.say(
            "The trail is over",
            "Everyone has gone home to rest. Maybe next time you'll make it all the way to Oregon!",
        );
    } else {
        g.say(
            "The trail is over",
            if g.t.mood == Mood::Survival {
                "Everyone in your party has died. Survival is the hardest trail of all."
            } else {
                "Everyone in your party has died. The trail to Oregon is a hard one."
            },
        );
    }
    Scene::GameOver
}

pub fn render_game_over(g: &Game, f: &mut Frame) {
    ui::header(f, "Game over");
    if g.t.mood == Mood::Gentle {
        pictures::backdrop(f, crate::data::Terrain::Plains, ui::HEAD_H, 66, 0, gfx::SKY);
        pictures::team(f, 100, 56, 0);
    } else {
        let leader = g.t.party.first().map_or("", |p| p.name.as_str());
        pictures::tomb(f, leader, ui::HEAD_H, 66);
    }
    f.text_center(88, "Try again? The", gfx::WHITE, Font::Normal);
    f.text_center(104, "trail is waiting.", gfx::WHITE, Font::Normal);
    ui::footer(f, "Press any key.");
}
