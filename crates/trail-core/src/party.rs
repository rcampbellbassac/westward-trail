//! The one-party games: one wagon with many drivers, democracy, and hunting
//! together. The host runs the only trip and shares its screen
//! ([`crate::stream`]); everyone else watches it and sends their keys back.
//!
//! Lines (on top of the ones in [`crate::net`]):
//!
//! ```text
//! scr <host> <row> <base64>          the host's screen, rows that changed
//! ask <host> <id>                    "send me the whole screen"
//! key <host> <id> <d|u> <code>       a key pressed (d) or let go (u) by a watcher
//! drv <host> <id>                    whose turn it is to drive
//! end <host> <text>                  the trip is over
//! ```
//!
//! Who may press what:
//! * **One wagon, many drivers:** only the driver. The turn passes to the next
//!   player each time the wagon sets off down the trail.
//! * **Democracy:** at a choice (the size-up menu, pace, rations, rivers,
//!   trades...) every player's number is a vote. The vote ends when everyone
//!   has voted, when the host's timer runs out, or (with no timer) when the
//!   leader presses ENTER. Most votes wins; on a tie the leader's vote wins.
//!   Players whose party member is gone are ghosts and can't vote. Anything
//!   else (typing, the store) is done by the leader, and anyone can turn the
//!   page of a message.
//! * **Hunting:** everyone has their own hunter in the same field.

use alloc::{format, string::String, vec::Vec};
use core::{cell::RefCell, mem::Discriminant};

use crate::{
    Game, Held, Key, Scene,
    gfx::{self, Font, Frame, H, W},
    net::{Mode, Screen},
    stream, travel, ui,
};

/// Without a picture from the host for this long, it's gone or out of reach.
const LOST_MS: u32 = 8_000;
/// Tell everyone whose turn it is this often, for anyone who missed it.
const DRIVER_MS: u32 = 2_500;

struct Vote {
    /// Which screen the vote is for; a different screen ends it.
    on: Discriminant<Scene>,
    keys: Vec<Key>,
    votes: Vec<(u32, Key)>,
    /// Milliseconds left, or None with no timer.
    left: Option<u32>,
}

#[derive(Default)]
pub struct Party {
    /// Whose turn it is to drive (one wagon, many drivers).
    pub driver: u32,
    vote: Option<Vote>,
    was_traveling: bool,
    driver_in: u32,
    // Host: the screen going out, filled in while drawing.
    pub(crate) tx: RefCell<stream::Sender>,
    pub(crate) out: RefCell<Vec<String>>,
    // Watchers: keys held and shots fired by each player, for the hunt.
    held: Vec<(u32, Held)>,
    pub(crate) shots: Vec<u32>,
    // Watching: the host's screen.
    screen: stream::Screen,
    seen: u32,
    asked: u32,
}

impl Party {
    pub fn held_of(&self, id: u32) -> Held {
        self.held
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, h)| *h)
            .unwrap_or_default()
    }
}

/// Key codes on the wire: the same numbers the web page uses.
pub fn key_code(k: Key) -> u32 {
    match k {
        Key::Backspace => 8,
        Key::Enter => 13,
        Key::Escape => 27,
        Key::Up => 0xE000,
        Key::Down => 0xE001,
        Key::Left => 0xE002,
        Key::Right => 0xE003,
        Key::Char(c) => c as u32,
    }
}

pub fn code_key(code: u32) -> Option<Key> {
    Some(match code {
        8 => Key::Backspace,
        13 => Key::Enter,
        27 => Key::Escape,
        0xE000 => Key::Up,
        0xE001 => Key::Down,
        0xE002 => Key::Left,
        0xE003 => Key::Right,
        c => Key::Char(char::from_u32(c)?),
    })
}

fn say(g: &mut Game, kind: &str, rest: &str) {
    if let Some(host) = g.net.host {
        g.net.send(format!("{kind} {host} {rest}"));
    }
}

/// Is this a one-party game we're in?
pub fn active(g: &Game) -> bool {
    g.net.racing && g.net.host.is_some() && g.net.settings.mode.one_party()
}

/// We're hosting a one-party game.
pub fn hosting(g: &Game) -> bool {
    active(g) && g.net.hosting()
}

/// We're watching someone else's one-party game.
pub fn watching(g: &Game) -> bool {
    active(g) && !g.net.hosting()
}

/// Players who can still vote: everyone except those whose party member is gone.
fn voters(g: &Game) -> Vec<u32> {
    g.net
        .players
        .iter()
        .enumerate()
        .filter(|(i, _)| g.t.party.get(*i).is_none_or(|p| !p.gone))
        .map(|(_, p)| p.id)
        .collect()
}

/// The keys that count as votes on this screen, or None if it isn't a choice.
fn vote_keys(g: &Game) -> Option<Vec<Key>> {
    let digits = |n: usize| -> Vec<Key> {
        (1..=n.min(9))
            .map(|i| Key::Char(char::from(b'0' + i as u8)))
            .collect()
    };
    Some(match &g.scene {
        Scene::SizeUp => {
            let mut keys = digits(travel::actions(g).len());
            keys.push(Key::Char('h'));
            keys.push(Key::Char('g'));
            if travel::can_fish(g) {
                keys.push(Key::Char('f'));
            }
            keys
        }
        Scene::Pace | Scene::Rations => digits(3),
        Scene::Dalles => digits(2),
        Scene::Occupation => digits(g.data.occupations.len()),
        Scene::River(r) if r.crossing.is_none() => digits(crate::river::options(g, r).len()),
        Scene::Trade(Some(_)) => alloc::vec![Key::Char('y'), Key::Char('n')],
        _ => return None,
    })
}

/// Letters vote the same in either case.
fn same_key(k: Key) -> Key {
    match k {
        Key::Char(c) => Key::Char(c.to_ascii_lowercase()),
        k => k,
    }
}

enum Rule {
    /// Press the key in the game.
    Apply,
    /// Count it as a vote.
    Vote,
    /// It moves that player's own hunter.
    Hunter,
    Ignore,
}

fn rule(g: &Game, id: u32) -> Rule {
    let leader = id == g.net.me;
    if let Scene::Hunt(h) = &g.scene
        && h.together()
        && g.msgs.is_empty()
    {
        return Rule::Hunter;
    }
    match g.net.settings.mode {
        Mode::Shared if id == g.net.party.driver => Rule::Apply,
        Mode::Shared => Rule::Ignore,
        Mode::Democracy if !g.msgs.is_empty() => Rule::Apply,
        Mode::Democracy if vote_keys(g).is_some() => Rule::Vote,
        _ if leader => Rule::Apply,
        _ => Rule::Ignore,
    }
}

/// A key on this device. True if it was dealt with here (sent, voted, or
/// ignored), false if the game should press it as usual.
pub fn local_key(g: &mut Game, k: Key, down: bool) -> bool {
    if !active(g) {
        return false;
    }
    if !g.net.hosting() {
        if !matches!(g.scene, Scene::Online(Screen::Remote)) {
            return false; // our own menu
        }
        if k == Key::Char('`') {
            if down {
                g.scene = Scene::Online(Screen::RemoteMenu);
            }
            return true;
        }
        let me = g.net.me;
        let ud = if down { "d" } else { "u" };
        say(g, "key", &format!("{me} {ud} {}", key_code(k)));
        return true;
    }
    let me = g.net.me;
    match rule(g, me) {
        Rule::Apply | Rule::Hunter => false,
        Rule::Vote => {
            if down {
                vote(g, me, k);
            }
            true
        }
        Rule::Ignore => {
            if down && g.net.settings.mode == Mode::Shared {
                let who = g.net.name_of(g.net.party.driver);
                g.notify(format!("It's {who}'s turn to drive"));
            }
            true
        }
    }
}

/// A key from a watcher, at the host.
pub fn remote_key(g: &mut Game, id: u32, down: bool, k: Key) {
    if !hosting(g) || !g.net.players.iter().any(|p| p.id == id) {
        return;
    }
    match rule(g, id) {
        Rule::Hunter => {
            let p = &mut g.net.party;
            let i = match p.held.iter().position(|(i, _)| *i == id) {
                Some(i) => i,
                None => {
                    p.held.push((id, Held::default()));
                    p.held.len() - 1
                }
            };
            crate::set_held(&mut p.held[i].1, k, down);
            if down && matches!(k, Key::Char(' ') | Key::Enter) {
                p.shots.push(id);
            }
        }
        Rule::Apply if down => g.press(k),
        Rule::Apply => g.release(k),
        Rule::Vote if down => vote(g, id, k),
        Rule::Ignore if down && g.net.settings.mode == Mode::Shared => {
            let who = g.net.name_of(g.net.party.driver);
            g.notify(format!("It's {who}'s turn to drive"));
        }
        _ => {}
    }
}

fn vote(g: &mut Game, id: u32, k: Key) {
    let Some(keys) = vote_keys(g) else {
        return;
    };
    let on = core::mem::discriminant(&g.scene);
    let k = same_key(k);
    let leader = id == g.net.me;
    if let Some(v) = &g.net.party.vote
        && leader
        && k == Key::Enter
        && v.left.is_none()
        && !v.votes.is_empty()
    {
        close_vote(g);
        return;
    }
    if !keys.contains(&k) {
        if leader {
            // The leader can still go back from a menu.
            g.press(k);
        }
        return;
    }
    if !voters(g).contains(&id) {
        let who = g.net.name_of(id);
        g.notify(format!("{who} is a ghost: cheer, don't vote!"));
        return;
    }
    let secs = g.net.settings.vote_secs;
    let v = g.net.party.vote.get_or_insert_with(|| Vote {
        on,
        keys,
        votes: Vec::new(),
        left: (secs > 0).then_some(secs * 1000),
    });
    v.votes.retain(|(i, _)| *i != id);
    v.votes.push((id, k));
    g.beep(700, 20);
    let all = voters(g);
    let done = g
        .net
        .party
        .vote
        .as_ref()
        .is_some_and(|v| all.iter().all(|id| v.votes.iter().any(|(i, _)| i == id)));
    if done {
        close_vote(g);
    }
}

fn close_vote(g: &mut Game) {
    let Some(v) = g.net.party.vote.take() else {
        return;
    };
    if v.votes.is_empty() {
        return;
    }
    let mut best: Option<(Key, usize)> = None;
    let mut tied = Vec::new();
    for &k in &v.keys {
        let n = v.votes.iter().filter(|(_, x)| *x == k).count();
        match best {
            Some((_, m)) if n < m => {}
            Some((_, m)) if n == m => tied.push(k),
            _ if n > 0 => {
                best = Some((k, n));
                tied = alloc::vec![k];
            }
            _ => {}
        }
    }
    let Some((mut winner, n)) = best else {
        return;
    };
    if tied.len() > 1 {
        // A tie: the leader's vote, if it's one of them, or else the first vote cast.
        let me = g.net.me;
        winner = v
            .votes
            .iter()
            .find(|(i, k)| *i == me && tied.contains(k))
            .or_else(|| v.votes.iter().find(|(_, k)| tied.contains(k)))
            .map_or(winner, |(_, k)| *k);
        g.notify(format!("Tie! The leader picks {}", key_name(winner)));
    } else {
        let s = if n == 1 { "" } else { "s" };
        g.notify(format!(
            "The party picks {} ({n} vote{s})",
            key_name(winner)
        ));
    }
    g.tune(&[(523, 40), (784, 60)]);
    g.press(winner);
    g.release(winner);
}

fn key_name(k: Key) -> String {
    match k {
        Key::Char(c) => String::from(c.to_ascii_uppercase()),
        _ => String::from("?"),
    }
}

// ---------------------------------------------------------------- every frame

pub fn tick(g: &mut Game, ms: u32) {
    if !active(g) {
        return;
    }
    let clock = g.clock;
    if !g.net.hosting() {
        let p = &mut g.net.party;
        if clock.wrapping_sub(p.seen) > 4_000 && clock.wrapping_sub(p.asked) > 4_000 {
            p.asked = clock;
            let me = g.net.me;
            say(g, "ask", &format!("{me}"));
        }
        return;
    }
    g.net.party.tx.get_mut().tick(ms);

    // A vote ends when the screen changes or the timer runs out.
    let on = core::mem::discriminant(&g.scene);
    if let Some(v) = &mut g.net.party.vote {
        if v.on != on {
            g.net.party.vote = None;
        } else if let Some(left) = &mut v.left {
            *left = left.saturating_sub(ms);
            if *left == 0 {
                close_vote(g);
            }
        }
    }

    if g.net.settings.mode == Mode::Shared {
        // The turn passes each time the wagon sets off, and when a driver leaves.
        let traveling = matches!(g.scene, Scene::Travel { .. });
        let set_off = traveling && !g.net.party.was_traveling;
        g.net.party.was_traveling = traveling;
        let gone = !g.net.players.iter().any(|p| p.id == g.net.party.driver);
        if set_off || gone {
            next_driver(g);
        }
        let p = &mut g.net.party;
        p.driver_in = p.driver_in.saturating_sub(ms);
        if p.driver_in == 0 {
            p.driver_in = DRIVER_MS;
            let d = p.driver;
            say(g, "drv", &format!("{d}"));
        }
    }
}

fn next_driver(g: &mut Game) {
    let n = &g.net;
    if n.players.is_empty() {
        return;
    }
    let i = n
        .players
        .iter()
        .position(|p| p.id == n.party.driver)
        .map_or(0, |i| (i + 1) % n.players.len());
    let id = n.players[i].id;
    let name = n.players[i].name.clone();
    g.net.party.driver = id;
    g.net.party.driver_in = 0;
    if id == g.net.me {
        g.notify("Your turn to drive!");
    } else {
        g.notify(format!("{name}'s turn to drive!"));
    }
    g.tune(&[(659, 50), (523, 50)]);
}

/// The host starts a one-party trip.
pub fn begin_host(g: &mut Game) {
    let me = g.net.me;
    let p = &mut g.net.party;
    p.driver = me;
    p.vote = None;
    p.was_traveling = false;
    p.driver_in = 0;
    p.held.clear();
    p.shots.clear();
    p.tx.get_mut().refresh();
}

/// A watcher joins a one-party trip.
pub fn begin_watching(g: &mut Game) -> Scene {
    g.net.party.screen = stream::Screen::default();
    g.net.party.seen = g.clock;
    g.net.party.asked = 0;
    g.msgs.clear();
    g.notify("Press ` (backtick) for your menu");
    Scene::Online(Screen::Remote)
}

// ---------------------------------------------------------------- lines in

/// Lines only the one-party games use. True if it was one of them.
pub fn receive(g: &mut Game, kind: &str, w: &[&str]) -> bool {
    let num = |i: usize| -> Option<u32> { w.get(i)?.parse().ok() };
    match kind {
        "scr" if watching(g) => {
            if let (Some(row), Some(b64)) = (num(2), w.get(3)) {
                g.net.party.screen.take(row as usize, b64);
                g.net.party.seen = g.clock;
            }
        }
        "ask" if hosting(g) => g.net.party.tx.get_mut().refresh(),
        "key" => {
            if let (Some(id), Some(ud), Some(k)) = (num(2), w.get(3), num(4).and_then(code_key)) {
                remote_key(g, id, *ud == "d", k);
            }
        }
        "drv" if watching(g) => {
            if let Some(id) = num(2) {
                if id != g.net.party.driver && id == g.net.me {
                    g.notify("Your turn to drive!");
                    g.tune(&[(659, 50), (523, 50)]);
                }
                g.net.party.driver = id;
            }
        }
        "end" if watching(g) => {
            let text = w.get(2..).map_or_else(String::new, |w| w.join(" "));
            crate::net::leave(g);
            g.say("The trip is over", text);
            g.pending = Some(Scene::Online(Screen::Menu));
        }
        _ => return false,
    }
    true
}

/// The host's trip is over: tell the watchers.
pub fn end(g: &mut Game, text: &str) {
    if hosting(g) {
        say(g, "end", text);
    }
}

// ---------------------------------------------------------------- drawing

/// On the host's screen (and so everyone's): the vote, or whose turn it is.
pub fn overlay(g: &Game, f: &mut Frame) {
    if !hosting(g) || g.net.players.len() < 2 {
        return;
    }
    let n = &g.net;
    match n.settings.mode {
        Mode::Shared => {
            let name = n.name_of(n.party.driver);
            let text = format!("Driver: {name}");
            let w = text.len() as i32 * 5 + 4;
            f.rect(W - w - 1, H - 22, w + 1, 10, gfx::BLACK);
            f.rect(W - w - 1, H - 22, 3, 10, n.color(n.party.driver));
            f.text(W - w + 3, H - 21, &text, gfx::YELLOW, Font::Small);
        }
        Mode::Democracy if g.msgs.is_empty() && vote_keys(g).is_some() => {
            let text = match &n.party.vote {
                None => String::from("Everyone votes! Press your choice."),
                Some(v) => {
                    let mut s = match v.left {
                        Some(ms) => format!("Vote {}s:", ms.div_ceil(1000)),
                        None => String::from("Vote (ENTER ends):"),
                    };
                    for k in &v.keys {
                        let c = v.votes.iter().filter(|(_, x)| x == k).count();
                        if c > 0 {
                            s.push_str(&format!(" {}={c}", key_name(*k)));
                        }
                    }
                    let waiting = voters(g)
                        .iter()
                        .filter(|id| !v.votes.iter().any(|(i, _)| i == *id))
                        .count();
                    if waiting > 0 {
                        s.push_str(&format!("  waiting for {waiting}"));
                    }
                    s
                }
            };
            f.rect(0, H - 22, W, 10, gfx::DARK_BLUE);
            f.text(3, H - 21, &text, gfx::WHITE, Font::Small);
        }
        _ => {}
    }
}

/// Hosting: send whatever changed on the screen.
pub fn share(g: &Game, f: &Frame) {
    if !hosting(g) || g.net.players.len() < 2 || !g.net.online {
        return;
    }
    let host = g.net.me;
    let lines = g.net.party.tx.borrow_mut().lines(host, f);
    let mut out = g.net.party.out.borrow_mut();
    // If the network falls behind, drop pictures rather than pile them up
    // (a Cardputer hasn't much memory to spare). The next full screen fixes it.
    let queued: usize = out.iter().map(String::len).sum();
    if queued < 32 * 1024 {
        out.extend(lines);
    }
}

/// Watching: the host's screen, with a frame around it when it's our turn.
pub fn render_remote(g: &Game, f: &mut Frame) {
    let p = &g.net.party;
    let host = g.net.host.map_or_else(String::new, |h| g.net.name_of(h));
    if p.screen.got == 0 {
        ui::header(f, "Playing together");
        f.text(
            6,
            30,
            &format!("Waiting for {host}'s screen..."),
            gfx::YELLOW,
            Font::Normal,
        );
        ui::para(
            f,
            6,
            50,
            "Your keys go to the game once it shows. Press ` (backtick) any time for your own menu.",
            gfx::GRAY,
        );
        return;
    }
    p.screen.draw(f);
    if g.net.settings.mode == Mode::Shared && p.driver == g.net.me {
        f.outline(0, 0, W, H, gfx::YELLOW);
        f.outline(1, 1, W - 2, H - 2, gfx::YELLOW);
    }
    if g.clock.wrapping_sub(p.seen) > LOST_MS {
        f.label(
            20,
            60,
            &format!("Lost touch with {host}..."),
            gfx::WHITE,
            gfx::RED,
            Font::Normal,
        );
    }
}
