//! Pixel art, drawn with letters. Each letter is one pixel:
//!
//!   .  see-through      k black     w white      a gray      A dark gray
//!   r  red              o orange    y yellow     g green     G dark green
//!   u  blue             U dark blue b brown      B dark brown
//!   t  tan              s skin      p purple
//!
//! Change a letter, rebuild, and the picture in the game changes.
//! Animals face RIGHT; the game mirrors them when they run left.

use crate::gfx::{self, Color};

pub struct Sprite {
    pub rows: &'static [&'static str],
}

impl Sprite {
    pub fn w(&self) -> i32 {
        self.rows.iter().map(|r| r.len()).max().unwrap_or(0) as i32
    }

    pub fn h(&self) -> i32 {
        self.rows.len() as i32
    }
}

pub fn palette(ch: u8) -> Option<Color> {
    Some(match ch {
        b'k' => gfx::BLACK,
        b'w' => gfx::WHITE,
        b'a' => gfx::GRAY,
        b'A' => gfx::DARK_GRAY,
        b'r' => gfx::RED,
        b'o' => gfx::ORANGE,
        b'y' => gfx::YELLOW,
        b'g' => gfx::GREEN,
        b'G' => gfx::DARK_GREEN,
        b'u' => gfx::BLUE,
        b'U' => gfx::DARK_BLUE,
        b'b' => gfx::BROWN,
        b'B' => gfx::DARK_BROWN,
        b't' => gfx::TAN,
        b's' => gfx::SKIN,
        b'p' => gfx::PURPLE,
        _ => return None,
    })
}

/// Covered wagon facing left (west!).
pub const WAGON: Sprite = Sprite {
    rows: &[
        "....wwwawwwwawwwww....",
        "...wwwwawwwwawwwwww...",
        "..wwwwwawwwwawwwwwww..",
        "..wwwwwawwwwawwwwwww..",
        "..wwwwwawwwwawwwwwww..",
        "..wwwwwawwwwawwwwwww..",
        ".bbbbbbbbbbbbbbbbbbbb.",
        ".bBBBBBBBBBBBBBBBBBBb.",
        "bbbbbbbbbbbbbbbbbbbbbb",
        "..kkk..........kkkk...",
        ".kBaBk........kBaaBk..",
        ".kaBak........kaBBak..",
        ".kBaBk........kBaaBk..",
        "..kkk..........kkkk...",
    ],
};

/// Ox facing left, two walking frames.
pub const OX: [Sprite; 2] = [
    Sprite {
        rows: &[
            "w.w...........",
            ".bb..bbbbbbbb.",
            "bkbbbbbbbbbbbb",
            "bbbbbbbbbbbbbb",
            "..bbbbbbbbbbbb",
            "...bbbbbbbbbb.",
            "...b.b....b.b.",
            "...k.k....k.k.",
        ],
    },
    Sprite {
        rows: &[
            "w.w...........",
            ".bb..bbbbbbbb.",
            "bkbbbbbbbbbbbb",
            "bbbbbbbbbbbbbb",
            "..bbbbbbbbbbbb",
            "...bbbbbbbbbb.",
            "....bb...bb...",
            "....kk...kk...",
        ],
    },
];

pub const HUNTER: Sprite = Sprite {
    rows: &[
        "..bbb...", ".bbbbb..", "..sss...", "..sks...", ".rrrrr..", "rrrrrrr.", "s.rrr.s.",
        "..rrr...", "..uuu...", "..u.u...", "..u.u...", ".kk.kk..",
    ],
};

pub const SQUIRREL: Sprite = Sprite {
    rows: &[".bb....", "b..b.bb", "b..bbkb", ".bbbbb.", "...b.b."],
};

pub const RABBIT: Sprite = Sprite {
    rows: &[
        "......a.", ".....aa.", ".aaaaaka", "waaaaaaa", ".aaaaaa.", ".a...a..",
    ],
};

pub const DEER: Sprite = Sprite {
    rows: &[
        "..........b.b.",
        "...........b..",
        "..........bbb.",
        ".........bbkbb",
        ".........bbbb.",
        ".bbbbbbbbbbb..",
        "wbbbbbbbbbbb..",
        ".bbbbbbbbbbb..",
        ".b.b.....b.b..",
        ".b.b.....b.b..",
        ".b.b.....b.b..",
        ".k.k.....k.k..",
    ],
};

pub const ELK: Sprite = Sprite {
    rows: &[
        "...........B..B.",
        "..........B.BB.B",
        "...........BBBB.",
        "............BB..",
        "...........BBB..",
        "..........BBkBB.",
        "..........BBBB..",
        ".tttttttttttB...",
        "ttttttttttttB...",
        ".tttttttttttB...",
        ".tttttttttt.....",
        ".B.B.....B.B....",
        ".B.B.....B.B....",
        ".k.k.....k.k....",
    ],
};

pub const BUFFALO: Sprite = Sprite {
    rows: &[
        ".........BBB......",
        "......BBBBBBB.....",
        "....BBBBBBBBBB....",
        "..BBBBBBBBBBBBBB..",
        ".BbbbbbbbbBBBBBkB.",
        "BbbbbbbbbbBBBBBBBw",
        "BbbbbbbbbbbBBBBBB.",
        ".bbbbbbbbbbbBBBB..",
        ".bbbbbbbbbbbb.....",
        "..b.b......b.b....",
        "..b.b......b.b....",
        "..k.k......k.k....",
    ],
};

pub const BEAR: Sprite = Sprite {
    rows: &[
        "..........BB.B.",
        "...BBBBB..BBBB.",
        ".BBBBBBBBBBBkB.",
        "BBBBBBBBBBBBBBk",
        "BBBBBBBBBBBBB..",
        "BBBBBBBBBBBB...",
        ".BBBBBBBBBBB...",
        ".BB.BB...BB.BB.",
        ".BB.BB...BB.BB.",
        ".kk.kk...kk.kk.",
    ],
};

pub const TREE: Sprite = Sprite {
    rows: &[
        "....GGG....",
        "...GgGGG...",
        "..GgggGGG..",
        "..GgggGGG..",
        ".GggggGGGG.",
        ".GgggGGGGG.",
        "GgggGGGGGGG",
        "GggGGGGGGGG",
        ".GGGGGGGGG.",
        "..GGGGGGG..",
        "....bb.....",
        "....bb.....",
        "....bb.....",
    ],
};

pub const BUSH: Sprite = Sprite {
    rows: &[".G.G.G..", "GaGGaGG.", "GGaGGaGG", ".GGGGGG.", "..b..b.."],
};

pub const ROCK: Sprite = Sprite {
    rows: &[
        "...aaaa...",
        "..aaaaaaA.",
        ".aaawaaaAA",
        "aaaaaaaaAA",
        "aaaaaaaAAA",
        ".AAAAAAAA.",
    ],
};

/// Raft seen from above, carrying the wagon.
pub const RAFT: Sprite = Sprite {
    rows: &[
        "bBbBbBbBbB",
        "bBwwwwwwbB",
        "bBwwwwwwbB",
        "bBwaaaawbB",
        "bBwwwwwwbB",
        "bBwaaaawbB",
        "bBwwwwwwbB",
        "bBwwwwwwbB",
        "bBbBbBbBbB",
    ],
};

pub const RIVER_ROCK: Sprite = Sprite {
    rows: &[
        "..aaaa...",
        ".aawaaaA.",
        "aawaaaaAA",
        "aaaaaaaAA",
        ".aaaaaAA.",
        "..AAAA...",
    ],
};

pub const FISH: Sprite = Sprite {
    rows: &[
        "...uuu....",
        ".uuuuuuu.u",
        "ukuuuuuuuu",
        ".uuuuuuu.u",
        "...uuu....",
    ],
};

pub const BOOT: Sprite = Sprite {
    rows: &[
        "..bbb..", "..bbb..", "..bbb..", "..bbb..", "bbbbbb.", "BBBBBBB",
    ],
};

pub fn animal(name: &str) -> &'static Sprite {
    match name {
        "squirrel" => &SQUIRREL,
        "deer" => &DEER,
        "elk" => &ELK,
        "buffalo" => &BUFFALO,
        "bear" => &BEAR,
        _ => &RABBIT,
    }
}
