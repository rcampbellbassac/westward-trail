//! TRAIL-OS: the secret hacker terminal. Press ` (backtick) to open it.
//! Want a new command? Add a line to `run` below, and to HELP.

use alloc::{boxed::Box, format, string::String, vec::Vec};

use crate::{
    Game, Key, Precip, Scene,
    gfx::{self, Font, Frame, Theme, W},
    hunt, raft, river,
};

const HELP: &[&str] = &[
    "give food|ammo|cash|oxen|cloth|parts N",
    "heal    god     ammo    peek    seed N",
    "speed 1-10    warp MILES    win",
    "weather clear|rain|snow|hot|cold",
    "theme color|green|amber",
    "hunt    fish    raft    forage",
    "ls  cat FILE  whoami  matrix  clear",
    "exit (or ESC)",
];

pub struct TermState {
    buf: String,
    lines: Vec<String>,
    history: Vec<String>,
    hist: usize,
    prev: Scene,
}

pub fn open(prev: Scene) -> Scene {
    let lines = alloc::vec![
        String::from("TRAIL-OS v1.848"),
        String::from("Type  help  for commands. ESC leaves."),
    ];
    Scene::Terminal(Box::new(TermState {
        buf: String::new(),
        lines,
        history: Vec::new(),
        hist: 0,
        prev,
    }))
}

const GREEN: gfx::Color = gfx::rgb(60, 255, 90);

pub fn render(g: &Game, f: &mut Frame, t: &TermState) {
    f.clear(gfx::rgb(0, 12, 0));
    let rows = 11;
    let start = t.lines.len().saturating_sub(rows);
    let mut y = 2;
    for line in &t.lines[start..] {
        f.text(2, y, line, GREEN, Font::Normal);
        y += 11;
    }
    f.rect(0, 122, W, 13, gfx::rgb(0, 40, 0));
    let cursor = if (g.clock / 350) % 2 == 0 { "#" } else { " " };
    f.text(
        2,
        124,
        &format!("> {}{cursor}", t.buf),
        gfx::WHITE,
        Font::Normal,
    );
    if g.cheats.used {
        f.text(W - 42, 125, "[HACKED]", gfx::YELLOW, Font::Small);
    }
}

pub fn key(g: &mut Game, mut t: Box<TermState>, k: Key) -> Scene {
    match k {
        Key::Escape | Key::Char('`' | '~') => return t.prev,
        Key::Backspace => {
            t.buf.pop();
        }
        Key::Up if !t.history.is_empty() => {
            t.hist = t.hist.saturating_sub(1);
            t.buf = t.history[t.hist].clone();
        }
        Key::Down if !t.history.is_empty() => {
            t.hist = (t.hist + 1).min(t.history.len());
            t.buf = t.history.get(t.hist).cloned().unwrap_or_default();
        }
        Key::Enter => {
            let cmd = core::mem::take(&mut t.buf);
            t.lines.push(format!("> {cmd}"));
            if !cmd.trim().is_empty() {
                t.history.push(cmd.clone());
                t.hist = t.history.len();
            }
            g.beep(1200, 15);
            if let Some(scene) = run(g, &mut t, cmd.trim()) {
                return scene;
            }
            if t.lines.len() > 60 {
                t.lines.drain(..20);
            }
        }
        Key::Char(c) if t.buf.len() < 28 => t.buf.push(c),
        _ => {}
    }
    Scene::Terminal(t)
}

fn out(t: &mut TermState, s: impl Into<String>) {
    t.lines.push(s.into());
}

/// Run one command. Returns a scene if the command leaves the terminal.
fn run(g: &mut Game, t: &mut TermState, cmd: &str) -> Option<Scene> {
    let words: Vec<&str> = cmd.split_whitespace().collect();
    let words: Vec<&str> = if words.first() == Some(&"sudo") {
        words[1..].to_vec()
    } else {
        words
    };
    let arg = |i: usize| words.get(i).copied().unwrap_or("");
    let num = |i: usize| arg(i).parse::<u32>().ok();
    let lower = arg(0).to_ascii_lowercase();
    match lower.as_str() {
        "" => {}
        "help" | "?" => {
            for h in HELP {
                out(t, *h);
            }
        }
        "clear" | "cls" => t.lines.clear(),
        "exit" | "quit" | "q" => return Some(core::mem::replace(&mut t.prev, Scene::Title)),
        "give" => {
            let n = num(2).unwrap_or(100);
            let s = &mut g.t.sup;
            let what = arg(1).to_ascii_lowercase();
            match what.as_str() {
                "food" => s.food = s.food.saturating_add(n),
                "ammo" | "bullets" => s.ammo = s.ammo.saturating_add(n),
                "cash" | "money" => s.cash = s.cash.saturating_add(n.saturating_mul(100)),
                "oxen" | "ox" => s.oxen = s.oxen.saturating_add(n),
                "clothing" | "clothes" | "cloth" => s.clothing = s.clothing.saturating_add(n),
                "parts" => {
                    s.wheels += n.min(99);
                    s.axles += n.min(99);
                    s.tongues += n.min(99);
                }
                _ => {
                    out(t, "give what? food ammo cash oxen clothing parts");
                    return None;
                }
            }
            g.cheats.used = true;
            out(t, format!("Added {n} {what}. Shhh..."));
        }
        "heal" => {
            for p in &mut g.t.party {
                p.sick = None;
            }
            g.t.health = 0;
            g.t.ox_lame = 0;
            g.cheats.used = true;
            out(t, "Everyone feels great! Oxen too.");
        }
        "god" => {
            g.cheats.god = !g.cheats.god;
            g.cheats.used = true;
            out(
                t,
                if g.cheats.god {
                    "God mode ON: nobody can leave the party."
                } else {
                    "God mode off."
                },
            );
        }
        "ammo" => {
            g.cheats.infinite_ammo = !g.cheats.infinite_ammo;
            g.cheats.used = true;
            out(
                t,
                if g.cheats.infinite_ammo {
                    "Infinite ammo ON."
                } else {
                    "Infinite ammo off."
                },
            );
        }
        "speed" => {
            let n = num(1).unwrap_or(1).clamp(1, 10);
            g.cheats.speed = n;
            out(t, format!("Travel speed x{n}."));
        }
        "warp" => {
            let n = num(1).unwrap_or(100);
            let limit = g
                .data
                .landmarks
                .get(g.t.next)
                .map_or(g.t.miles, |l| l.mile.saturating_sub(1));
            if matches!(t.prev, Scene::SizeUp | Scene::Travel { .. }) {
                g.t.miles = (g.t.miles + n).min(limit).max(g.t.miles);
                g.t.at = None;
                g.cheats.used = true;
                t.prev = Scene::Travel { acc_ms: 0 };
                out(
                    t,
                    format!("Warped to mile {}. (Can't skip landmarks!)", g.t.miles),
                );
            } else {
                out(t, "You can only warp while on the trail.");
            }
        }
        "win" => {
            if g.t.party.is_empty() {
                out(t, "Start a trip first!");
            } else {
                g.cheats.used = true;
                return Some(river::finish_trail(g));
            }
        }
        "weather" => {
            match arg(1) {
                "clear" => g.t.precip = Precip::Clear,
                "rain" => g.t.precip = Precip::Rain,
                "snow" => {
                    g.t.precip = Precip::Snow;
                    g.t.temp = g.t.temp.min(30);
                }
                "hot" => g.t.temp = 95,
                "cold" => g.t.temp = 10,
                _ => {
                    out(t, "weather clear|rain|snow|hot|cold");
                    return None;
                }
            }
            out(t, "The sky obeys you.");
        }
        "theme" => {
            g.cheats.theme = match arg(1) {
                "green" => Theme::Green,
                "amber" => Theme::Amber,
                _ => Theme::Color,
            };
            out(t, "Monitor adjusted.");
        }
        "peek" | "stats" => {
            g.cheats.peek = !g.cheats.peek;
            out(
                t,
                format!("health points {} (0 = perfect, 140+ = deadly)", g.t.health),
            );
            out(
                t,
                format!(
                    "temp {}F  ox lame {} days  day {}",
                    g.t.temp, g.t.ox_lame, g.t.day
                ),
            );
            out(
                t,
                format!("miles {}  next landmark #{}", g.t.miles, g.t.next),
            );
            out(
                t,
                format!(
                    "peek on size-up screen: {}",
                    if g.cheats.peek { "on" } else { "off" }
                ),
            );
        }
        "seed" => match num(1) {
            Some(n) => {
                g.rng = crate::rng::Rng::new(u64::from(n));
                out(t, format!("Random seed set to {n}. Same seed = same luck."));
            }
            None => out(t, "seed NUMBER"),
        },
        "hunt" => {
            g.cheats.used = true;
            return Some(Scene::Hunt(Box::new(hunt::HuntState::new(g))));
        }
        "fish" => return Some(Scene::Fish(Box::default())),
        "forage" | "gather" => {
            g.cheats.used = true;
            return Some(crate::forage::start(g));
        }
        "raft" => {
            if g.t.party.is_empty() {
                out(t, "Start a trip first!");
            } else {
                g.cheats.used = true;
                return Some(Scene::Raft(Box::new(raft::RaftState::new(g))));
            }
        }
        "ls" | "dir" => out(t, "trail.txt  sprites.rs  terminal.rs  secrets/"),
        "cat" | "type" => match arg(1) {
            "trail.txt" => {
                let d = &g.data;
                out(
                    t,
                    format!(
                        "{} landmarks, {} animals, {} fish",
                        d.landmarks.len(),
                        d.animals.len(),
                        d.fish.len()
                    ),
                );
                out(t, format!("trail is {} miles long", d.trail_length()));
                out(t, "Edit it in the web Mod editor!");
            }
            "sprites.rs" => out(t, "Pictures drawn with letters. b=brown, w=white..."),
            "terminal.rs" => out(t, "That's me! Add your own commands in run()."),
            "secrets" | "secrets/" => out(t, "Permission denied. Nice try, hacker."),
            _ => out(t, "cat: no such file"),
        },
        "whoami" => {
            let leader =
                g.t.party
                    .first()
                    .map_or("a mysterious stranger", |p| p.name.as_str());
            out(t, format!("{leader}. Also: a hacker, obviously."));
        }
        "matrix" => {
            for i in 0..10u32 {
                let line: String = (0..38)
                    .map(|_| (b'!' + (g.rng.below(90) as u8)) as char)
                    .collect();
                let _ = i;
                out(t, line);
            }
            out(t, "Wake up, pioneer...");
        }
        "hello" | "hi" => out(t, "Hello! Type help."),
        "xyzzy" => out(t, "A hollow voice says: \"Wrong game.\""),
        _ => out(t, format!("{lower}: command not found. Try help.")),
    }
    None
}
