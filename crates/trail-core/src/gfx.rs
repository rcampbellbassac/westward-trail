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
    pub fn text(&mut self, x: i32, y: i32, s: &str, c: Color, font: Font) -> i32 {
        let style = MonoTextStyle::new(font.mono(), c);
        let _ = Text::with_baseline(s, Point::new(x, y), style, Baseline::Top).draw(self);
        s.chars().count() as i32 * font.char_w()
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
