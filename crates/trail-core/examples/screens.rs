//! Save screenshots of the main screens as PPM images:
//!   cargo run -p trail-core --example screens -- /tmp/shots
//! Handy for checking art changes without a browser or a Cardputer.

use std::{fs, path::Path};

use embedded_graphics::prelude::IntoStorage;

use trail_core::{
    Game, Key,
    gfx::{self, Frame, H, W},
};

fn save(g: &Game, dir: &Path, name: &str) {
    let mut f = Frame::new();
    g.render(&mut f);
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    f.to_rgba(&mut rgba);
    let mut out = format!("P6\n{W} {H}\n255\n").into_bytes();
    for px in rgba.chunks(4) {
        out.extend_from_slice(&px[..3]);
    }
    fs::write(dir.join(format!("{name}.ppm")), out).expect("write screenshot");
}

fn keys(g: &mut Game, ks: &[Key]) {
    for &k in ks {
        g.key_down(k);
        g.key_up(k);
    }
}

fn typed(g: &mut Game, s: &str) {
    for c in s.chars() {
        keys(g, &[Key::Char(c)]);
    }
    keys(g, &[Key::Enter]);
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| String::from("screens"));
    let dir = Path::new(&dir);
    fs::create_dir_all(dir).expect("create output dir");
    let mut g = Game::new(3);
    g.tick(1200);
    save(&g, dir, "01_title");
    let mut f = Frame::new();
    g.render(&mut f);
    fs::write(dir.join("01_title.bmp"), f.to_bmp()).expect("write bmp");
    keys(&mut g, &[Key::Char('1'), Key::Char('2'), Key::Char('2')]);
    typed(&mut g, "Sam");
    keys(&mut g, &[Key::Enter, Key::Enter]);
    save(&g, dir, "02_names");
    keys(
        &mut g,
        &[Key::Enter, Key::Enter, Key::Char('y'), Key::Char('2')],
    );
    save(&g, dir, "03_message");
    keys(&mut g, &[Key::Enter, Key::Enter]);
    for (item, qty) in [('1', "3"), ('2', "800"), ('3', "8"), ('4', "4"), ('5', "1")] {
        keys(&mut g, &[Key::Char(item)]);
        typed(&mut g, qty);
    }
    save(&g, dir, "04_store");
    keys(&mut g, &[Key::Enter, Key::Enter]);
    save(&g, dir, "05_sizeup");
    keys(&mut g, &[Key::Char('1')]);
    for _ in 0..12 {
        g.tick(100);
    }
    save(&g, dir, "06_travel");
    keys(&mut g, &[Key::Escape, Key::Char('3')]);
    save(&g, dir, "07_map");
    keys(&mut g, &[Key::Enter, Key::Char('h')]);
    for i in 0..60 {
        if i == 20 {
            g.key_down(Key::Right);
        }
        if i % 9 == 0 {
            keys(&mut g, &[Key::Char(' ')]);
        }
        g.tick(80);
    }
    g.key_up(Key::Right);
    save(&g, dir, "08_hunt");
    keys(&mut g, &[Key::Escape]);
    keys(&mut g, &[Key::Char('`')]);
    for cmd in ["help", "give food 500"] {
        typed(&mut g, cmd);
    }
    save(&g, dir, "09_terminal");
    typed(&mut g, "fish");
    keys(&mut g, &[Key::Char(' ')]);
    g.tick(400);
    save(&g, dir, "10_fish");
    keys(&mut g, &[Key::Escape, Key::Enter, Key::Char('`')]);
    typed(&mut g, "raft");
    for _ in 0..30 {
        g.tick(100);
    }
    save(&g, dir, "11_raft");

    // A second trip for the newer screens: gathering and the save slots.
    let mut g = Game::new(8);
    keys(&mut g, &[Key::Char('1'), Key::Char('1'), Key::Char('3')]);
    typed(&mut g, "Sam");
    keys(
        &mut g,
        &[Key::Enter, Key::Enter, Key::Enter, Key::Enter, Key::Enter],
    );
    keys(
        &mut g,
        &[Key::Char('y'), Key::Char('2'), Key::Enter, Key::Enter],
    );
    for (item, qty) in [('1', "3"), ('2', "600")] {
        keys(&mut g, &[Key::Char(item)]);
        typed(&mut g, qty);
    }
    keys(&mut g, &[Key::Enter, Key::Enter]);
    save(&g, dir, "12_sizeup_new");
    keys(&mut g, &[Key::Char('g')]);
    save(&g, dir, "13_gather_warning");
    keys(&mut g, &[Key::Enter, Key::Enter]);
    // Wander until the forager stands next to a plant (its name label shows up).
    let label = gfx::rgb(20, 40, 15).into_storage();
    let mut f = Frame::new();
    let dirs = [Key::Left, Key::Up, Key::Right, Key::Down];
    'walk: for leg in 0..40usize {
        let k = dirs[leg % 4];
        g.key_down(k);
        for _ in 0..(3 + leg * 2) {
            g.tick(60);
            g.render(&mut f);
            if f.px.contains(&label) {
                g.key_up(k);
                keys(&mut g, &[Key::Char(' ')]);
                g.tick(200);
                break 'walk;
            }
        }
        g.key_up(k);
    }
    save(&g, dir, "14_gather");
    keys(&mut g, &[Key::Escape]);
    save(&g, dir, "15_gather_done");
    keys(
        &mut g,
        &[
            Key::Enter,
            Key::Enter,
            Key::Enter,
            Key::Char('s'),
            Key::Char('2'),
        ],
    );
    keys(&mut g, &[Key::Char('s')]);
    save(&g, dir, "16_slots");
    println!("saved screenshots to {}", dir.display());
}
