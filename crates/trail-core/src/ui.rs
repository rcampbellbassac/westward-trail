//! Shared screen pieces: title bars, menus, message boxes, formatting.

use alloc::{format, string::String};

use crate::{
    Game, Key, Msg, Pic,
    gfx::{self, Font, Frame, W},
    pictures,
};

pub const COLS: usize = 39;
const MSG_TEXT_TOP: i32 = 16;
const MSG_PIC_TEXT_TOP: i32 = 76;
const FOOTER_Y: i32 = 126;

pub fn header(f: &mut Frame, title: &str) {
    f.rect(0, 0, W, 12, gfx::TAN);
    f.text(3, 1, title, gfx::BLACK, Font::Normal);
}

pub fn footer(f: &mut Frame, s: &str) {
    f.text(3, FOOTER_Y, s, gfx::GRAY, Font::Small);
}

/// Numbered menu. Returns the y below the last line.
pub fn menu(f: &mut Frame, x: i32, mut y: i32, items: &[&str]) -> i32 {
    for (i, item) in items.iter().enumerate() {
        f.text(
            x,
            y,
            &format!("{}. {}", i + 1, item),
            gfx::WHITE,
            Font::Normal,
        );
        y += 10;
    }
    y
}

/// Wrapped paragraph. Returns the y below it.
pub fn para(f: &mut Frame, x: i32, mut y: i32, text: &str, c: gfx::Color) -> i32 {
    let cols = ((W - x - 2) / Font::Normal.char_w()) as usize;
    for line in gfx::wrap(text, cols) {
        f.text(x, y, &line, c, Font::Normal);
        y += 10;
    }
    y
}

/// The number key pressed, 1-based, if any.
pub fn digit(k: Key) -> Option<usize> {
    match k {
        Key::Char(c @ '1'..='9') => Some(c as usize - '0' as usize),
        _ => None,
    }
}

pub fn yes(k: Key) -> bool {
    matches!(k, Key::Char('y' | 'Y'))
}

pub fn no(k: Key) -> bool {
    matches!(k, Key::Char('n' | 'N') | Key::Escape)
}

pub fn money(cents: u32) -> String {
    format!("${}.{:02}", cents / 100, cents % 100)
}

pub const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

fn month_lengths(year: u32) -> [u32; 12] {
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
    ]
}

/// (month 0-11, day of month) for a day of the year, rolling into next year.
pub fn month_day(year: u32, mut day: u32) -> (usize, u32, u32) {
    let mut y = year;
    loop {
        let lens = month_lengths(y);
        let total: u32 = lens.iter().sum();
        if day <= total {
            for (m, len) in lens.iter().enumerate() {
                if day <= *len {
                    return (m, day, y);
                }
                day -= len;
            }
        }
        day -= total;
        y += 1;
    }
}

pub fn date(g: &Game) -> String {
    let (m, d, y) = month_day(g.data.year, g.t.day.max(1));
    format!("{} {}, {}", MONTHS[m], d, y)
}

pub fn month_index(g: &Game) -> usize {
    month_day(g.data.year, g.t.day.max(1)).0
}

pub fn health_word(h: u32) -> &'static str {
    match h {
        0..35 => "good",
        35..70 => "fair",
        70..105 => "poor",
        _ => "very poor",
    }
}

pub fn weather_word(g: &Game) -> &'static str {
    use crate::Precip;
    match (g.t.precip, g.t.temp) {
        (Precip::Snow, t) if t < 15 => "blizzard",
        (Precip::Snow, _) => "snowy",
        (Precip::Rain, _) => "rainy",
        (_, t) if t < 20 => "very cold",
        (_, t) if t < 40 => "cold",
        (_, t) if t < 60 => "cool",
        (_, t) if t < 80 => "warm",
        (_, t) if t < 90 => "hot",
        _ => "very hot",
    }
}

pub fn pace_word(p: usize) -> &'static str {
    ["steady", "strenuous", "grueling"][p.min(2)]
}

pub fn rations_word(r: usize) -> &'static str {
    ["filling", "meager", "bare bones"][r.min(2)]
}

fn msg_lines(m: &Msg) -> (alloc::vec::Vec<String>, usize) {
    let lines = gfx::wrap(&m.body, COLS);
    let cap = if matches!(m.pic, Pic::None) { 10 } else { 5 };
    (lines, cap)
}

pub fn msg_pages(m: &Msg) -> usize {
    let (lines, cap) = msg_lines(m);
    lines.len().div_ceil(cap).max(1)
}

pub fn render_msg(g: &Game, f: &mut Frame, m: &Msg) {
    header(f, &m.title);
    let (lines, cap) = msg_lines(m);
    let mut y = if matches!(m.pic, Pic::None) {
        MSG_TEXT_TOP
    } else {
        MSG_PIC_TEXT_TOP
    };
    match &m.pic {
        Pic::None => {}
        Pic::Landmark(i) => pictures::landmark(g, f, *i, 12, 62),
        Pic::Tomb(name) => pictures::tomb(f, name, 12, 62),
    }
    for line in lines.iter().skip(m.page * cap).take(cap) {
        f.text(3, y, line, gfx::WHITE, Font::Normal);
        y += 10;
    }
    let more = (m.page + 1) * cap < lines.len();
    footer(
        f,
        if more {
            "ENTER: more..."
        } else {
            "Press ENTER to continue"
        },
    );
}
