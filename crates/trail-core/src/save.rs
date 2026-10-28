//! Saved trips: five slots, each a small text file you can read (and hack).
//!
//! The game only turns a trip into text and back. Where the text lives
//! (browser storage, the Cardputer's flash, the SD card) is up to the platform:
//! it hands saved slots in with [`Game::import_slot`] and writes the ones in
//! [`Game::take_saves`].

use alloc::{format, string::String, vec::Vec};

use crate::{
    Game, Key, Mood, Person, Precip, Scene, Trip,
    gfx::{self, Font, Frame},
    ui,
};

pub const SLOTS: usize = 5;
const VERSION: u32 = 1;

/// Something the platform should write down.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SaveOp {
    /// Slot 0-4 and its text.
    Slot(usize, String),
    /// The Top Ten, as from [`Game::export_scores`].
    Scores(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SlotMode {
    Save,
    Load,
}

pub struct SlotState {
    pub mode: SlotMode,
    /// A slot waiting for "replace it? Y/N".
    pub confirm: Option<usize>,
}

/// Which trail a save belongs to, so a save from a modded trail isn't loaded into another.
fn trail_id(g: &Game) -> String {
    format!(
        "{}|{}|{}",
        g.data.title,
        g.data.landmarks.len(),
        g.data.trail_length()
    )
}

fn clean(s: &str) -> String {
    s.replace(['|', '\n', '\r'], "/")
}

pub fn to_text(g: &Game) -> String {
    let t = &g.t;
    let place = t.at.map_or_else(
        || format!("{} miles", t.miles),
        |i| g.data.landmarks[i].name.clone(),
    );
    let leader = t.party.first().map_or("Pioneer", |p| p.name.as_str());
    let mut s = String::from(
        "# Westward Trail saved trip. It's just text: change a number, load it, see what happens.\n",
    );
    let mut kv = |k: &str, v: String| s.push_str(&format!("{k} = {v}\n"));
    kv("version", format!("{VERSION}"));
    kv("saved", format!("{}", g.save_count));
    kv("trail", clean(&trail_id(g)));
    kv(
        "summary",
        clean(&format!("{leader}, {}, {place}", ui::date(g))),
    );
    kv(
        "mood",
        String::from(if t.mood == Mood::Gentle {
            "gentle"
        } else {
            "classic"
        }),
    );
    kv("occupation", format!("{}", t.occupation));
    for p in &t.party {
        let (sick, days) = p.sick.clone().unwrap_or_default();
        let state = if p.gone { "gone" } else { "here" };
        kv(
            "person",
            format!("{}|{state}|{}|{days}", clean(&p.name), clean(&sick)),
        );
    }
    let u = &t.sup;
    for (k, v) in [
        ("cash", u.cash),
        ("oxen", u.oxen),
        ("food", u.food),
        ("clothing", u.clothing),
        ("ammo", u.ammo),
        ("wheels", u.wheels),
        ("axles", u.axles),
        ("tongues", u.tongues),
        ("day", t.day),
        ("days", t.days),
        ("miles", t.miles),
        ("next", t.next as u32),
        ("pace", t.pace as u32),
        ("rations", t.rations as u32),
        ("health", t.health),
        ("ox_lame", t.ox_lame),
    ] {
        kv(k, format!("{v}"));
    }
    kv(
        "at",
        t.at.map_or_else(|| String::from("none"), |i| format!("{i}")),
    );
    kv("temp", format!("{}", t.temp));
    let precip = match t.precip {
        Precip::Clear => "clear",
        Precip::Rain => "rain",
        Precip::Snow => "snow",
    };
    kv("precip", String::from(precip));
    kv(
        "hacker",
        String::from(if g.cheats.used { "yes" } else { "no" }),
    );
    kv("rng", format!("{}", g.rng.state()));
    s
}

/// How many saves this game had made when the text was written: bigger is newer.
/// The Cardputer has no clock, so this decides which copy of a slot is newest.
pub fn sequence(text: &str) -> u32 {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("saved"))
        .and_then(|rest| rest.trim_start().strip_prefix('='))
        .and_then(|v| v.trim().parse().ok())
        .unwrap_or(0)
}

/// The "summary = ..." line, for the slot list.
pub fn summary(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|l| l.trim().strip_prefix("summary"))
        .and_then(|rest| rest.trim_start().strip_prefix('='))
        .map(str::trim)
}

/// Rebuild a trip from saved text. Errors are short and friendly.
pub fn from_text(g: &Game, text: &str) -> Result<(Trip, bool, u64), String> {
    let mut t = Trip::new();
    t.party.clear();
    let mut hacker = false;
    let mut rng = 1;
    let mut trail_ok = false;
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            return Err(format!("line {}: expected  key = value", n + 1));
        };
        let (k, v) = (k.trim(), v.trim());
        let bad = || format!("line {}: \"{v}\" doesn't look right for {k}", n + 1);
        let num = || v.parse::<u32>().map_err(|_| bad());
        match k {
            "version" if num()? > VERSION => {
                return Err(String::from("this save is from a newer version"));
            }
            "trail" => trail_ok = v == clean(&trail_id(g)),
            "mood" => {
                t.mood = if v == "gentle" {
                    Mood::Gentle
                } else {
                    Mood::Classic
                }
            }
            "occupation" => t.occupation = (num()? as usize).min(g.data.occupations.len() - 1),
            "person" => {
                let mut parts = v.split('|');
                let name = String::from(parts.next().unwrap_or("Pioneer"));
                let gone = parts.next() == Some("gone");
                let sick = parts.next().unwrap_or("");
                let days = parts.next().and_then(|d| d.parse().ok()).unwrap_or(0);
                t.party.push(Person {
                    name,
                    sick: (!sick.is_empty()).then(|| (String::from(sick), days)),
                    gone,
                });
            }
            "cash" => t.sup.cash = num()?,
            "oxen" => t.sup.oxen = num()?,
            "food" => t.sup.food = num()?,
            "clothing" => t.sup.clothing = num()?,
            "ammo" => t.sup.ammo = num()?,
            "wheels" => t.sup.wheels = num()?,
            "axles" => t.sup.axles = num()?,
            "tongues" => t.sup.tongues = num()?,
            "day" => t.day = num()?.max(1),
            "days" => t.days = num()?,
            "miles" => t.miles = num()?,
            "next" => t.next = num()? as usize,
            "pace" => t.pace = (num()? as usize).min(2),
            "rations" => t.rations = (num()? as usize).min(2),
            "health" => t.health = num()?,
            "ox_lame" => t.ox_lame = num()?,
            "at" => {
                t.at = if v == "none" {
                    None
                } else {
                    Some(num()? as usize)
                }
            }
            "temp" => t.temp = v.parse().map_err(|_| bad())?,
            "precip" => {
                t.precip = match v {
                    "rain" => Precip::Rain,
                    "snow" => Precip::Snow,
                    _ => Precip::Clear,
                }
            }
            "hacker" => hacker = v == "yes",
            "rng" => rng = v.parse().map_err(|_| bad())?,
            _ => {} // unknown keys are ignored, so old games can read newer saves a bit
        }
    }
    if !trail_ok {
        return Err(String::from(
            "this trip was saved on a different trail (a mod?). Load that trail first.",
        ));
    }
    let last = g.data.landmarks.len() - 1;
    if t.party.is_empty() || t.next == 0 || t.next > last || t.at.is_some_and(|i| i > last) {
        return Err(String::from("this save is missing something important"));
    }
    t.miles = t.miles.min(g.data.trail_length());
    Ok((t, hacker, rng))
}

pub fn open(mode: SlotMode) -> Scene {
    Scene::Slots(SlotState {
        mode,
        confirm: None,
    })
}

fn back(mode: SlotMode) -> Scene {
    match mode {
        SlotMode::Save => Scene::SizeUp,
        SlotMode::Load => Scene::Title,
    }
}

fn write(g: &mut Game, slot: usize) -> Scene {
    g.save_count += 1;
    let text = to_text(g);
    g.slots[slot] = Some(text.clone());
    g.saves.push(SaveOp::Slot(slot, text));
    g.notify(format!("Saved in slot {}", slot + 1));
    g.tune(&[(523, 60), (659, 60), (784, 90)]);
    Scene::SizeUp
}

pub fn key(g: &mut Game, s: SlotState, k: Key) -> Scene {
    if k == Key::Escape {
        return back(s.mode);
    }
    if let Some(slot) = s.confirm {
        if ui::yes(k) {
            return write(g, slot);
        }
        if ui::no(k) {
            return Scene::Slots(SlotState { confirm: None, ..s });
        }
        return Scene::Slots(s);
    }
    let Some(n) = ui::digit(k).filter(|n| (1..=SLOTS).contains(n)) else {
        return Scene::Slots(s);
    };
    let slot = n - 1;
    match s.mode {
        SlotMode::Save if g.slots[slot].is_some() => Scene::Slots(SlotState {
            confirm: Some(slot),
            ..s
        }),
        SlotMode::Save => write(g, slot),
        SlotMode::Load => {
            let Some(text) = g.slots[slot].clone() else {
                g.beep(150, 60);
                return Scene::Slots(s);
            };
            match from_text(g, &text) {
                Ok((trip, hacker, rng)) => {
                    g.t = trip;
                    g.cheats.used = hacker;
                    g.rng = crate::rng::Rng::from_state(rng);
                    g.msgs.clear();
                    let leader = g.t.party.first().map_or("", |p| p.name.as_str());
                    let body = format!(
                        "Welcome back, {leader}! It's {}. The trail is waiting.",
                        ui::date(g)
                    );
                    g.say("Trip loaded", body);
                    g.beep(659, 60);
                    Scene::SizeUp
                }
                Err(e) => {
                    g.say("Can't load that trip", format!("Slot {n}: {e}"));
                    Scene::Slots(s)
                }
            }
        }
    }
}

pub fn render(g: &Game, f: &mut Frame, s: &SlotState) {
    let title = match s.mode {
        SlotMode::Save => "Save your trip",
        SlotMode::Load => "Load a saved trip",
    };
    ui::header(f, title);
    for i in 0..SLOTS {
        let y = 18 + i as i32 * 18;
        let (label, color) = match g.slots[i].as_deref() {
            Some(text) => (
                String::from(summary(text).unwrap_or("A saved trip")),
                gfx::WHITE,
            ),
            None => (String::from("(empty)"), gfx::GRAY),
        };
        f.text(6, y, &format!("{}.", i + 1), gfx::YELLOW, Font::Normal);
        let lines = gfx::wrap(&label, 36);
        for (n, line) in lines.iter().take(2).enumerate() {
            f.text(
                24,
                y + n as i32 * 8,
                line,
                color,
                if lines.len() > 1 {
                    Font::Small
                } else {
                    Font::Normal
                },
            );
        }
    }
    let footer = match (s.mode, s.confirm) {
        (_, Some(slot)) => format!("Replace slot {}? Y or N", slot + 1),
        (SlotMode::Save, None) => String::from("Pick a slot 1-5.  ESC: back"),
        (SlotMode::Load, None) => String::from("Pick a trip 1-5.  ESC: back"),
    };
    ui::footer(f, &footer);
}

/// Quietly wrap up a save op list for tests and platforms.
pub fn take(g: &mut Game) -> Vec<SaveOp> {
    core::mem::take(&mut g.saves)
}
