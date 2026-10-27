//! Scenery painted with simple shapes: landmark pictures, backdrops, tombstones.

use crate::{
    Game,
    data::Terrain,
    gfx::{self, Color, Font, Frame, W},
    sprites,
};

pub fn isqrt(n: i32) -> i32 {
    if n <= 0 {
        return 0;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

/// Filled triangle "mountain" with an optional snowcap.
pub fn mountain(f: &mut Frame, cx: i32, peak: i32, base: i32, half_w: i32, c: Color, snow: bool) {
    let h = (base - peak).max(1);
    for y in peak..base {
        let half = half_w * (y - peak) / h;
        let col = if snow && y - peak < h / 4 {
            gfx::WHITE
        } else {
            c
        };
        f.rect(cx - half, y, half * 2 + 1, 1, col);
    }
}

/// Filled half-ellipse sitting on `base`.
pub fn dome(f: &mut Frame, cx: i32, base: i32, rx: i32, ry: i32, c: Color) {
    for dx in -rx..=rx {
        let hh = isqrt(rx * rx - dx * dx) * ry / rx.max(1);
        f.rect(cx + dx, base - hh, 1, hh, c);
    }
}

pub fn ground_color(t: Terrain) -> Color {
    match t {
        Terrain::Plains => gfx::rgb(110, 170, 70),
        Terrain::Hills => gfx::rgb(120, 150, 70),
        Terrain::Mountains => gfx::rgb(110, 120, 90),
        Terrain::Desert => gfx::rgb(210, 180, 120),
        Terrain::Forest => gfx::rgb(60, 120, 60),
    }
}

/// Sky, distant land and ground for a band of the screen.
/// `scroll` slides the distant scenery so the wagon looks like it moves.
pub fn backdrop(f: &mut Frame, terrain: Terrain, top: i32, h: i32, scroll: i32, sky: Color) {
    let horizon = top + h * 3 / 5;
    f.rect(0, top, W, horizon - top, sky);
    f.rect(0, horizon, W, top + h - horizon, ground_color(terrain));
    let far = scroll / 3;
    match terrain {
        Terrain::Plains => {
            for i in -1..6 {
                let x = (i * 60 + far).rem_euclid(300) - 30;
                dome(f, x, horizon, 30, 5, gfx::rgb(90, 150, 60));
            }
        }
        Terrain::Hills => {
            for i in -1..5 {
                let x = (i * 70 + far).rem_euclid(350) - 40;
                dome(f, x, horizon, 40, 12, gfx::rgb(100, 135, 60));
            }
        }
        Terrain::Mountains => {
            for i in -1..5 {
                let x = (i * 64 + far).rem_euclid(320) - 40;
                let peak = horizon - 22 - (i.rem_euclid(3)) * 6;
                mountain(
                    f,
                    x,
                    peak.max(top + 2),
                    horizon,
                    34,
                    gfx::rgb(120, 110, 140),
                    true,
                );
            }
        }
        Terrain::Desert => {
            for i in -1..4 {
                let x = (i * 90 + far).rem_euclid(360) - 40;
                f.rect(x, horizon - 10, 30, 10, gfx::rgb(190, 120, 80));
                f.rect(x + 4, horizon - 14, 22, 4, gfx::rgb(190, 120, 80));
            }
        }
        Terrain::Forest => {
            for i in -2..14 {
                let x = (i * 22 + far).rem_euclid(330) - 30;
                mountain(f, x, horizon - 16, horizon, 7, gfx::rgb(30, 90, 50), false);
            }
        }
    }
    // A few near tufts that move faster than the far hills.
    for i in 0..8 {
        let x = (i * 37 + scroll).rem_euclid(W + 20) - 10;
        let y = horizon + 4 + (i * 13) % (top + h - horizon - 4).max(1);
        f.rect(x, y, 3, 1, gfx::rgb(60, 100, 40));
    }
}

/// Full-width picture of a landmark between `top` and `top + h`.
pub fn landmark(g: &Game, f: &mut Frame, idx: usize, top: i32, h: i32) {
    let Some(l) = g.data.landmarks.get(idx) else {
        return;
    };
    let base = top + h * 3 / 5;
    backdrop(f, l.terrain, top, h, idx as i32 * 50, gfx::SKY);
    match l.picture.as_str() {
        "town" => {
            for (i, x) in [30, 70, 110, 150, 190].into_iter().enumerate() {
                let bh = 14 + (i as i32 % 3) * 5;
                let c = [gfx::BROWN, gfx::TAN, gfx::rgb(170, 70, 60)][i % 3];
                f.rect(x, base - bh, 26, bh, c);
                mountain(
                    f,
                    x + 13,
                    base - bh - 8,
                    base - bh,
                    15,
                    gfx::DARK_BROWN,
                    false,
                );
                f.rect(x + 10, base - 8, 6, 8, gfx::DARK_BROWN);
            }
            f.sprite(8, base + 4, &sprites::WAGON, false);
        }
        "fort" => {
            f.rect(50, base - 18, 140, 18, gfx::BROWN);
            for x in (50..190).step_by(5) {
                f.rect(x, base - 21, 3, 4, gfx::BROWN);
                f.rect(x + 3, base - 18, 1, 18, gfx::DARK_BROWN);
            }
            for x in [44, 178] {
                f.rect(x, base - 28, 18, 28, gfx::DARK_BROWN);
                f.rect(x + 2, base - 24, 14, 3, gfx::BLACK);
            }
            f.rect(112, base - 12, 16, 12, gfx::BLACK);
            f.rect(119, base - 42, 1, 24, gfx::WHITE);
            f.rect(120, base - 42, 12, 4, gfx::RED);
            f.rect(120, base - 38, 12, 4, gfx::WHITE);
            f.rect(120, base - 42, 5, 4, gfx::BLUE);
        }
        "river" => {
            f.rect(0, base + 2, W, 14, gfx::BLUE);
            for i in 0..14 {
                let x = (i * 31 + (g.clock / 80) as i32) % W;
                f.rect(x, base + 4 + (i % 3) * 4, 6, 1, gfx::SKY);
            }
            for x in [10, 40, 190, 215] {
                f.sprite(x, base - 13, &sprites::TREE, false);
            }
        }
        "chimney" => {
            mountain(f, 120, base - 22, base, 36, gfx::TAN, false);
            f.rect(115, base - 50, 10, 30, gfx::TAN);
            f.rect(117, base - 54, 6, 4, gfx::TAN);
        }
        "rock" => {
            dome(f, 120, base, 60, 24, gfx::rgb(170, 150, 120));
            dome(f, 105, base - 6, 25, 10, gfx::rgb(190, 170, 140));
        }
        "mountains" => {
            mountain(f, 60, top + 4, base, 60, gfx::rgb(110, 100, 130), true);
            mountain(f, 150, top + 10, base, 70, gfx::rgb(100, 90, 120), true);
            mountain(f, 215, top + 16, base, 40, gfx::rgb(120, 110, 140), true);
        }
        "springs" => {
            dome(f, 120, base + 12, 50, 8, gfx::rgb(80, 170, 200));
            for i in 0..10 {
                let x = 80 + i * 8;
                let y = base - ((g.clock / 60) as i32 + i * 7) % 20;
                f.rect(x, y, 2, 2, gfx::WHITE);
            }
        }
        "valley" => {
            f.rect(0, base, W, top + h - base, gfx::rgb(70, 180, 70));
            dome(f, 200, top + 14, 10, 10, gfx::YELLOW);
            f.rect(150, base - 14, 28, 14, gfx::rgb(170, 70, 60));
            mountain(f, 164, base - 26, base - 14, 17, gfx::DARK_BROWN, false);
            for x in (10..120).step_by(8) {
                f.rect(x, base + 6, 4, 6, gfx::YELLOW);
            }
        }
        _ => {
            dome(f, 120, base, 40, 16, gfx::GRAY);
        }
    }
    f.label(3, top + 2, &l.name, gfx::WHITE, gfx::BLACK, Font::Small);
}

/// Oxen in front (west, to the left), wagon behind. `y` is the wagon's top.
pub fn team(f: &mut Frame, x: i32, y: i32, step: usize) {
    f.sprite(x, y + 6, &sprites::OX[step % 2], false);
    f.rect(x + 13, y + 10, 6, 1, gfx::DARK_BROWN);
    f.sprite(x + 18, y, &sprites::WAGON, false);
}

/// Tiny landmark on the horizon while you travel toward it.
pub fn mini_landmark(f: &mut Frame, picture: &str, x: i32, base: i32) {
    match picture {
        "fort" => {
            f.rect(x, base - 8, 20, 8, gfx::BROWN);
            f.rect(x - 2, base - 11, 5, 11, gfx::DARK_BROWN);
            f.rect(x + 17, base - 11, 5, 11, gfx::DARK_BROWN);
        }
        "river" => f.rect(x, base - 2, 16, 30, gfx::BLUE),
        "chimney" => {
            mountain(f, x + 6, base - 8, base, 10, gfx::TAN, false);
            f.rect(x + 4, base - 20, 4, 12, gfx::TAN);
        }
        "mountains" => mountain(
            f,
            x + 10,
            base - 24,
            base,
            20,
            gfx::rgb(110, 100, 130),
            true,
        ),
        "town" | "valley" => {
            f.rect(x, base - 8, 8, 8, gfx::BROWN);
            f.rect(x + 10, base - 10, 8, 10, gfx::TAN);
        }
        "springs" => dome(f, x + 8, base, 10, 3, gfx::rgb(80, 170, 200)),
        _ => dome(f, x + 10, base, 14, 8, gfx::rgb(170, 150, 120)),
    }
}

pub fn tomb(f: &mut Frame, name: &str, top: i32, h: i32) {
    let base = top + h - 8;
    f.rect(0, top, W, h, gfx::rgb(40, 40, 70));
    f.rect(0, base, W, top + h - base, gfx::rgb(70, 110, 50));
    dome(f, 120, base - 30, 22, 12, gfx::GRAY);
    f.rect(98, base - 30, 45, 30, gfx::GRAY);
    f.text(111, base - 34, "RIP", gfx::BLACK, Font::Normal);
    let w = name.chars().count() as i32 * 5;
    f.text(120 - w / 2, base - 20, name, gfx::BLACK, Font::Small);
}
