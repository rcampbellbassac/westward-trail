//! WebAssembly exports used by `web/main.js`.
//!
//! No wasm-bindgen: plain `extern "C"` functions plus buffers in wasm memory,
//! so the web build is a single `cargo build --target wasm32-unknown-unknown`.

use std::cell::RefCell;

use trail_core::{
    Game, Key, SaveOp,
    data::DEFAULT_DATA,
    gfx::{Frame, H, W},
};

struct App {
    game: Game,
    frame: Frame,
    rgba: Vec<u8>,
    text_in: Vec<u8>,
    text_out: String,
    beeps: Vec<u32>,
    saves: Vec<SaveOp>,
    seed: u64,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut App) -> R) -> R {
    APP.with(|cell| {
        let mut slot = cell.borrow_mut();
        let app = slot.get_or_insert_with(|| App {
            game: Game::new(1),
            frame: Frame::new(),
            rgba: vec![0; (W * H * 4) as usize],
            text_in: Vec::new(),
            text_out: String::new(),
            beeps: Vec::new(),
            saves: Vec::new(),
            seed: 1,
        });
        f(app)
    })
}

/// Special key codes sent by main.js (printable keys send their character code).
fn decode(code: u32) -> Option<Key> {
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

#[unsafe(no_mangle)]
pub extern "C" fn trail_init(seed: u32) {
    with(|a| {
        a.seed = u64::from(seed);
        let scores = a.game.export_scores();
        a.game = Game::new(a.seed);
        a.game.import_scores(&scores);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn trail_width() -> u32 {
    W as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn trail_height() -> u32 {
    H as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn trail_key(code: u32, down: u32) {
    let Some(k) = decode(code) else { return };
    with(|a| {
        if down != 0 {
            a.game.key_down(k)
        } else {
            a.game.key_up(k)
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn trail_tick(ms: u32) {
    with(|a| {
        a.game.tick(ms);
        a.beeps.extend(
            a.game
                .take_beeps()
                .iter()
                .map(|b| (u32::from(b.hz) << 16) | u32::from(b.ms)),
        );
    });
}

/// Draw the frame and return a pointer to W*H RGBA pixels.
#[unsafe(no_mangle)]
pub extern "C" fn trail_render() -> *const u8 {
    with(|a| {
        a.game.render(&mut a.frame);
        a.frame.to_rgba(&mut a.rgba);
        a.rgba.as_ptr()
    })
}

/// Number of queued notes; read each with `trail_beep`, then they are cleared.
#[unsafe(no_mangle)]
pub extern "C" fn trail_beep_count() -> u32 {
    with(|a| a.beeps.len() as u32)
}

/// Note `i` packed as (hz << 16) | ms.
#[unsafe(no_mangle)]
pub extern "C" fn trail_beep(i: u32) -> u32 {
    with(|a| a.beeps.get(i as usize).copied().unwrap_or(0))
}

#[unsafe(no_mangle)]
pub extern "C" fn trail_beeps_clear() {
    with(|a| a.beeps.clear());
}

/// Reserve `len` bytes for text coming from JavaScript and return where to write it.
#[unsafe(no_mangle)]
pub extern "C" fn trail_text_in(len: u32) -> *mut u8 {
    with(|a| {
        a.text_in.clear();
        a.text_in.resize(len as usize, 0);
        a.text_in.as_mut_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn trail_text_out_ptr() -> *const u8 {
    with(|a| a.text_out.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn trail_text_out_len() -> u32 {
    with(|a| a.text_out.len() as u32)
}

/// Put the built-in data file in the text-out buffer.
#[unsafe(no_mangle)]
pub extern "C" fn trail_default_data() {
    with(|a| a.text_out = DEFAULT_DATA.to_string());
}

/// Restart with the modded data in the text-in buffer.
/// Returns 1 on success; on failure returns 0 and puts the error in text-out.
#[unsafe(no_mangle)]
pub extern "C" fn trail_load_mod() -> u32 {
    with(|a| {
        let src = String::from_utf8_lossy(&a.text_in).into_owned();
        match Game::with_data(&src, a.seed) {
            Ok(mut game) => {
                game.import_scores(&a.game.export_scores());
                a.game = game;
                a.text_out = String::from("Mod loaded! New trail is ready.");
                1
            }
            Err(e) => {
                a.text_out = e.to_string();
                0
            }
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn trail_export_scores() {
    with(|a| a.text_out = a.game.export_scores());
}

#[unsafe(no_mangle)]
pub extern "C" fn trail_import_scores() {
    with(|a| {
        let s = String::from_utf8_lossy(&a.text_in).into_owned();
        a.game.import_scores(&s);
    });
}

/// Hand in a saved trip from browser storage (text in the text-in buffer).
#[unsafe(no_mangle)]
pub extern "C" fn trail_import_slot(slot: u32) {
    with(|a| {
        let s = String::from_utf8_lossy(&a.text_in).into_owned();
        a.game.import_slot(slot as usize, &s);
    });
}

/// The next thing to write to browser storage: returns the slot 0-4, or 100 for
/// the Top Ten, with the text in text-out. Returns -1 when there's nothing left.
#[unsafe(no_mangle)]
pub extern "C" fn trail_next_save() -> i32 {
    with(|a| {
        a.saves.extend(a.game.take_saves());
        if a.saves.is_empty() {
            return -1;
        }
        match a.saves.remove(0) {
            SaveOp::Slot(slot, text) => {
                a.text_out = text;
                slot as i32
            }
            SaveOp::Scores(text) => {
                a.text_out = text;
                100
            }
        }
    })
}

/// Show the text-in buffer as a short note over the game.
#[unsafe(no_mangle)]
pub extern "C" fn trail_notify() {
    with(|a| {
        let s = String::from_utf8_lossy(&a.text_in).into_owned();
        a.game.notify(s);
    });
}
