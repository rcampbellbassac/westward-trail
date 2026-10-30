//! The 240x135 frame every platform shows, plus drawing helpers.
//! 240x135 is the Cardputer's screen; the web build scales it up.

use alloc::{string::String, vec, vec::Vec};
use embedded_graphics::{
    mono_font::{
        MonoFont, MonoTextStyle,
        ascii::{FONT_5X8, FONT_6X10, FONT_6X13_BOLD, FONT_10X20},
    },
    pixelcolor::{Rgb565, raw::RawU16},
    prelude::*,
    text::{Baseline, Text},
};

use crate::sprites::Sprite;

pub const W: i32 = 240;
pub const H: i32 = 135;
/// Bytes in a screenshot BMP: a 54-byte header and 3 bytes per pixel.
pub const BMP_SIZE: usize = 54 + (W * H * 3) as usize;

pub type Color = Rgb565;

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Rgb565::new(r >> 3, g >> 2, b >> 3)
}

/// Turn a raw RGB565 value from [`Frame::px`] back into a color (for display drivers).
pub fn color_from_raw(raw: u16) -> Color {
    Rgb565::from(RawU16::new(raw))
}

pub const BLACK: Color = rgb(0, 0, 0);
pub const WHITE: Color = rgb(255, 255, 255);
pub const GRAY: Color = rgb(150, 150, 150);
pub const DARK_GRAY: Color = rgb(70, 70, 70);
pub const RED: Color = rgb(230, 60, 50);
pub const ORANGE: Color = rgb(240, 150, 40);
pub const YELLOW: Color = rgb(250, 220, 70);
pub const GREEN: Color = rgb(80, 200, 70);
pub const DARK_GREEN: Color = rgb(30, 110, 40);
pub const BLUE: Color = rgb(70, 130, 230);
pub const DARK_BLUE: Color = rgb(30, 60, 140);
pub const SKY: Color = rgb(120, 180, 240);
pub const BROWN: Color = rgb(150, 95, 45);
pub const DARK_BROWN: Color = rgb(90, 55, 25);
pub const TAN: Color = rgb(220, 190, 130);
pub const SKIN: Color = rgb(240, 190, 150);
pub const PURPLE: Color = rgb(170, 90, 200);

/// Each number key has its own color, so children who can't read yet can match a menu line to
/// its key ("press the blue one"). The web keyboard uses the same colors (web/main.js).
pub const KEY_COLORS: [Color; 9] = [
    rgb(90, 160, 255),  // 1 blue
    rgb(250, 220, 70),  // 2 yellow
    rgb(255, 120, 190), // 3 pink
    rgb(185, 130, 255), // 4 purple
    rgb(60, 210, 200),  // 5 teal
    rgb(250, 150, 50),  // 6 orange
    rgb(200, 140, 80),  // 7 brown
    rgb(235, 235, 235), // 8 white
    rgb(160, 160, 160), // 9 gray
];
/// ENTER and Y: go ahead.
pub const KEY_GO: Color = rgb(80, 210, 90);
/// ESC and N: stop, go back.
pub const KEY_STOP: Color = rgb(240, 80, 70);
/// SPACE: shoot, cast, pick.
pub const KEY_SPACE: Color = rgb(235, 235, 235);

/// Where text names a key, so it can be drawn on a colored key cap:
/// a menu number at the start ("2. Learn"), ENTER, ESC, SPACE, and Y/N questions.
/// Returns (first char, chars, color) for each.
fn key_marks(s: &str) -> Vec<(usize, usize, Color)> {
    let c: Vec<char> = s.chars().collect();
    let mut marks = Vec::new();
    let start = c.iter().take_while(|&&ch| ch == ' ').count();
    if let Some(d @ '1'..='9') = c.get(start).copied()
        && c.get(start + 1) == Some(&'.')
        && matches!(c.get(start + 2), None | Some(' '))
    {
        marks.push((start, 1, KEY_COLORS[d as usize - '1' as usize]));
    }
    let word = |i: usize, w: &str| {
        let n = w.len();
        let edge = |j: Option<&char>| !j.is_some_and(|ch| ch.is_ascii_alphanumeric());
        c.len() >= i + n
            && c[i..i + n].iter().copied().eq(w.chars())
            && edge(i.checked_sub(1).and_then(|j| c.get(j)))
            && edge(c.get(i + n))
    };
    for i in 0..c.len() {
        if word(i, "ENTER") {
            marks.push((i, 5, KEY_GO));
        } else if word(i, "ESC") {
            marks.push((i, 3, KEY_STOP));
        } else if word(i, "SPACE") {
            marks.push((i, 5, KEY_SPACE));
        } else if word(i, "Y/N") {
            marks.push((i, 1, KEY_GO));
            marks.push((i + 2, 1, KEY_STOP));
        } else if word(i, "Y or N") {
            marks.push((i, 1, KEY_GO));
            marks.push((i + 5, 1, KEY_STOP));
        }
    }
    marks
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Font {
    Small,
    Normal,
    Bold,
    Big,
}

impl Font {
    fn mono(self) -> &'static MonoFont<'static> {
        match self {
            Font::Small => &FONT_5X8,
            Font::Normal => &FONT_6X10,
            Font::Bold => &FONT_6X13_BOLD,
            Font::Big => &FONT_10X20,
        }
    }

    pub fn char_w(self) -> i32 {
        self.mono().character_size.width as i32
    }

    pub fn line_h(self) -> i32 {
        self.mono().character_size.height as i32
    }
}

/// Color filters the hacker terminal can switch on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    Color,
    Green,
    Amber,
}

pub struct Frame {
    pub px: Vec<u16>,
}

impl Default for Frame {
    fn default() -> Self {
        Self::new()
    }
}

impl Frame {
    pub fn new() -> Self {
        Self {
            px: vec![0; (W * H) as usize],
        }
    }

    pub fn clear(&mut self, c: Color) {
        let raw = RawU16::from(c).into_inner();
        self.px.fill(raw);
    }

    pub fn pixel(&mut self, x: i32, y: i32, c: Color) {
        if (0..W).contains(&x) && (0..H).contains(&y) {
            self.px[(y * W + x) as usize] = RawU16::from(c).into_inner();
        }
    }

    pub fn rect(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        let raw = RawU16::from(c).into_inner();
        let (x0, y0) = (x.max(0), y.max(0));
        let (x1, y1) = ((x + w).min(W), (y + h).min(H));
        for yy in y0..y1 {
            let row = (yy * W) as usize;
            self.px[row + x0 as usize..row + x1.max(x0) as usize].fill(raw);
        }
    }

    pub fn outline(&mut self, x: i32, y: i32, w: i32, h: i32, c: Color) {
        self.rect(x, y, w, 1, c);
        self.rect(x, y + h - 1, w, 1, c);
        self.rect(x, y, 1, h, c);
        self.rect(x + w - 1, y, 1, h, c);
    }

    /// Draw text with its top-left corner at (x, y). Returns the width drawn.
    /// Keys named in the text (menu numbers, ENTER, ESC, SPACE, Y/N) get colored key caps.
    pub fn text(&mut self, x: i32, y: i32, s: &str, c: Color, font: Font) -> i32 {
        let cw = font.char_w();
        let marks = key_marks(s);
        for &(i, n, cap) in &marks {
            self.rect(
                x + i as i32 * cw - 1,
                y - 1,
                n as i32 * cw + 1,
                font.line_h() + 1,
                cap,
            );
        }
        let style = MonoTextStyle::new(font.mono(), c);
        let _ = Text::with_baseline(s, Point::new(x, y), style, Baseline::Top).draw(self);
        let ink = MonoTextStyle::new(font.mono(), BLACK);
        for &(i, n, _) in &marks {
            let part: String = s.chars().skip(i).take(n).collect();
            let at = Point::new(x + i as i32 * cw, y);
            let _ = Text::with_baseline(&part, at, ink, Baseline::Top).draw(self);
        }
        s.chars().count() as i32 * cw
    }

    pub fn text_center(&mut self, y: i32, s: &str, c: Color, font: Font) {
        let w = s.chars().count() as i32 * font.char_w();
        self.text((W - w) / 2, y, s, c, font);
    }

    /// Text with a solid background bar behind it.
    pub fn label(&mut self, x: i32, y: i32, s: &str, fg: Color, bg: Color, font: Font) {
        let w = s.chars().count() as i32 * font.char_w();
        self.rect(x - 1, y, w + 2, font.line_h(), bg);
        self.text(x, y, s, fg, font);
    }

    pub fn sprite(&mut self, x: i32, y: i32, s: &Sprite, flip_x: bool) {
        self.sprite_ex(x, y, s, flip_x, false, None);
    }

    /// Draw a sprite, optionally mirrored, flipped upside down, or all one color.
    pub fn sprite_ex(
        &mut self,
        x: i32,
        y: i32,
        s: &Sprite,
        flip_x: bool,
        flip_y: bool,
        tint: Option<Color>,
    ) {
        let (w, h) = (s.w(), s.h());
        for (row, line) in s.rows.iter().enumerate() {
            for (col, ch) in line.bytes().enumerate() {
                let Some(c) = crate::sprites::palette(ch) else {
                    continue;
                };
                let dx = if flip_x {
                    w - 1 - col as i32
                } else {
                    col as i32
                };
                let dy = if flip_y {
                    h - 1 - row as i32
                } else {
                    row as i32
                };
                self.pixel(x + dx, y + dy, tint.unwrap_or(c));
            }
        }
    }

    /// Draw a sprite whose `x` pixels take the given color (berries, caps, flowers).
    pub fn sprite_accent(&mut self, x: i32, y: i32, s: &Sprite, flip_x: bool, accent: Color) {
        let w = s.w();
        for (row, line) in s.rows.iter().enumerate() {
            for (col, ch) in line.bytes().enumerate() {
                let c = if ch == b'x' {
                    accent
                } else if let Some(c) = crate::sprites::palette(ch) {
                    c
                } else {
                    continue;
                };
                let dx = if flip_x {
                    w - 1 - col as i32
                } else {
                    col as i32
                };
                self.pixel(x + dx, y + row as i32, c);
            }
        }
    }

    /// The frame as a 24-bit BMP file, which any computer can open.
    pub fn to_bmp(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(BMP_SIZE);
        self.write_bmp(|chunk| out.extend_from_slice(chunk));
        out
    }

    /// The same BMP, handed out a piece at a time, so a small device can
    /// write it to a file without holding the whole picture in memory.
    pub fn write_bmp(&self, mut out: impl FnMut(&[u8])) {
        const ROW: usize = (W * 3) as usize; // 720 bytes: a multiple of 4, so no padding
        let mut header = [0u8; 54];
        header[..2].copy_from_slice(b"BM");
        let fields: [(usize, u32); 7] = [
            (2, BMP_SIZE as u32),
            (10, 54),
            (14, 40),
            (18, W as u32),
            (22, H as u32),
            (34, (ROW * H as usize) as u32),
            (38, 2835),
        ];
        for (at, v) in fields {
            header[at..at + 4].copy_from_slice(&v.to_le_bytes());
        }
        header[42..46].copy_from_slice(&2835u32.to_le_bytes());
        header[26] = 1; // planes
        header[28] = 24; // bits per pixel
        out(&header);
        // BMP rows go bottom to top, each pixel blue, green, red.
        let mut row = [0u8; ROW];
        for y in (0..H as usize).rev() {
            for x in 0..W as usize {
                let p = self.px[y * W as usize + x];
                let (r, g, b) = (
                    ((p >> 11) & 0x1F) as u8,
                    ((p >> 5) & 0x3F) as u8,
                    (p & 0x1F) as u8,
                );
                row[x * 3..x * 3 + 3].copy_from_slice(&[
                    (b << 3) | (b >> 2),
                    (g << 2) | (g >> 4),
                    (r << 3) | (r >> 2),
                ]);
            }
            out(&row);
        }
    }

    /// Recolor the whole frame like an old monochrome monitor.
    pub fn apply_theme(&mut self, theme: Theme) {
        let (r, g, b) = match theme {
            Theme::Color => return,
            Theme::Green => (40u32, 255u32, 80u32),
            Theme::Amber => (255, 176, 0),
        };
        for p in &mut self.px {
            let c = Rgb565::from(RawU16::new(*p));
            let luma = (u32::from(c.r()) * 8 * 77
                + u32::from(c.g()) * 4 * 150
                + u32::from(c.b()) * 8 * 29)
                >> 8;
            let luma = luma.min(255);
            let out = rgb(
                (r * luma / 255) as u8,
                (g * luma / 255) as u8,
                (b * luma / 255) as u8,
            );
            *p = RawU16::from(out).into_inner();
        }
    }

    /// RGBA8888 copy for the browser canvas.
    pub fn to_rgba(&self, out: &mut [u8]) {
        for (i, p) in self.px.iter().enumerate() {
            let r = ((p >> 11) & 0x1F) as u8;
            let g = ((p >> 5) & 0x3F) as u8;
            let b = (p & 0x1F) as u8;
            let o = i * 4;
            if o + 3 >= out.len() {
                break;
            }
            out[o] = (r << 3) | (r >> 2);
            out[o + 1] = (g << 2) | (g >> 4);
            out[o + 2] = (b << 3) | (b >> 2);
            out[o + 3] = 255;
        }
    }
}

impl DrawTarget for Frame {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(p, c) in pixels {
            self.pixel(p.x, p.y, c);
        }
        Ok(())
    }
}

impl OriginDimensions for Frame {
    fn size(&self) -> Size {
        Size::new(W as u32, H as u32)
    }
}

/// Word-wrap text to `cols` characters. Honors '\n'.
pub fn wrap(s: &str, cols: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let needed = if line.is_empty() {
                word.len()
            } else {
                line.len() + 1 + word.len()
            };
            if needed > cols && !line.is_empty() {
                out.push(core::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_named_in_text_get_caps() {
        assert_eq!(
            key_marks("2. Learn about the trail"),
            [(0, 1, KEY_COLORS[1])]
        );
        assert_eq!(key_marks("  5. Abe"), [(2, 1, KEY_COLORS[4])]);
        assert_eq!(
            key_marks("ENTER: continue  ESC: back"),
            [(0, 5, KEY_GO), (17, 3, KEY_STOP)]
        );
        assert_eq!(
            key_marks("Are these names correct? (Y/N)"),
            [(26, 1, KEY_GO), (28, 1, KEY_STOP)]
        );
        // Not keys: years, ranges, words that only contain a key's name.
        assert!(key_marks("1848. Independence").is_empty());
        assert!(key_marks("Pick a trip 1-5.").is_empty());
        assert!(key_marks("ESCAPED the river, ENTERED Oregon").is_empty());
    }
}
