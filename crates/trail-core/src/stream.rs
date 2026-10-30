//! Sharing the screen: how one game shows its screen on everyone else's.
//!
//! In the one-party games (one wagon with many drivers, democracy, hunting
//! together) only the host runs the trip. It sends its screen as text lines and
//! the others draw it, sending their keys back. Only rows that changed are sent,
//! plus every row now and then for anyone who missed some.
//!
//! A row is run-length coded: runs of (count - 1, color low byte, color high
//! byte), 3 bytes each, until the row's 240 pixels are covered. Rows that follow
//! each other go together, in base64:
//!
//! ```text
//! scr <host> <first row> <base64 of the rows>
//! ```

use alloc::{string::String, vec, vec::Vec};

use crate::gfx::{Frame, H, W};

/// Lines are kept under this many characters (relays allow 2048).
const LINE_MAX: usize = 1500;
/// Send at most this often.
const FRAME_MS: u32 = 80;
/// Send every row this often, for anyone who missed some.
const FULL_MS: u32 = 2500;

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(bytes: &[u8], out: &mut String) {
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
}

pub fn b64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

fn encode_row(px: &[u16], out: &mut Vec<u8>) {
    let mut i = 0;
    while i < px.len() {
        let c = px[i];
        let mut n = 1;
        while i + n < px.len() && px[i + n] == c && n < 256 {
            n += 1;
        }
        out.extend_from_slice(&[(n - 1) as u8, c as u8, (c >> 8) as u8]);
        i += n;
    }
}

/// A quick fingerprint of a row (FNV-1a), to see whether it changed.
fn row_hash(px: &[u16]) -> u32 {
    px.iter().fold(0x811c_9dc5, |h, &p| {
        (h ^ u32::from(p)).wrapping_mul(0x0100_0193)
    })
}

/// The sending side: remembers what it sent, as one fingerprint per row.
pub struct Sender {
    sent: Vec<u32>,
    wait: u32,
    full_in: u32,
}

impl Default for Sender {
    fn default() -> Self {
        Sender {
            sent: vec![0; H as usize],
            wait: 0,
            full_in: 0,
        }
    }
}

impl Sender {
    /// Send every row next time.
    pub fn refresh(&mut self) {
        self.full_in = 0;
    }

    pub fn tick(&mut self, ms: u32) {
        self.wait = self.wait.saturating_sub(ms);
        self.full_in = self.full_in.saturating_sub(ms);
    }

    /// The lines for this frame (none if it's too soon or nothing changed).
    pub fn lines(&mut self, host: u32, f: &Frame) -> Vec<String> {
        if self.wait > 0 {
            return Vec::new();
        }
        self.wait = FRAME_MS;
        let full = self.full_in == 0;
        if full {
            self.full_in = FULL_MS;
        }
        let mut lines = Vec::new();
        let mut bytes: Vec<u8> = Vec::new();
        let mut first: Option<usize> = None;
        let flush = |first: &mut Option<usize>, bytes: &mut Vec<u8>, lines: &mut Vec<String>| {
            if let Some(y) = first.take() {
                let mut line = alloc::format!("scr {host} {y} ");
                b64_encode(bytes, &mut line);
                lines.push(line);
                bytes.clear();
            }
        };
        for y in 0..H as usize {
            let row = &f.px[y * W as usize..(y + 1) * W as usize];
            let h = row_hash(row);
            if !full && h == self.sent[y] {
                flush(&mut first, &mut bytes, &mut lines);
                continue;
            }
            self.sent[y] = h;
            let before = bytes.len();
            encode_row(row, &mut bytes);
            if first.is_some() && bytes.len() * 4 / 3 > LINE_MAX {
                // Too long: send what came before, and start a new line here.
                let this: Vec<u8> = bytes.split_off(before);
                flush(&mut first, &mut bytes, &mut lines);
                bytes = this;
            }
            first.get_or_insert(y);
        }
        flush(&mut first, &mut bytes, &mut lines);
        lines
    }
}

/// The watching side: the host's screen, kept as coded rows.
#[derive(Default)]
pub struct Screen {
    rows: Vec<Vec<u8>>,
    /// Frames seen so far, so the watcher knows a picture has arrived.
    pub got: u32,
}

impl Screen {
    /// Take in one `scr` line's rows (from its first row number and base64 text).
    pub fn take(&mut self, first: usize, b64: &str) {
        let Some(bytes) = b64_decode(b64) else {
            return;
        };
        if self.rows.len() < H as usize {
            self.rows.resize(H as usize, Vec::new());
        }
        let mut y = first;
        let mut start = 0;
        let mut covered = 0usize;
        let mut i = 0;
        while i + 3 <= bytes.len() && y < H as usize {
            covered += usize::from(bytes[i]) + 1;
            i += 3;
            if covered >= W as usize {
                self.rows[y] = bytes[start..i].to_vec();
                y += 1;
                start = i;
                covered = 0;
            }
        }
        self.got = self.got.wrapping_add(1);
    }

    pub fn draw(&self, f: &mut Frame) {
        for (y, row) in self.rows.iter().enumerate() {
            let line = &mut f.px[y * W as usize..(y + 1) * W as usize];
            let mut x = 0;
            for run in row.as_chunks::<3>().0 {
                let n = usize::from(run[0]) + 1;
                let c = u16::from(run[1]) | (u16::from(run[2]) << 8);
                let end = (x + n).min(W as usize);
                line[x..end].fill(c);
                x = end;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gfx;

    #[test]
    fn base64_round_trip() {
        for len in 0..10 {
            let bytes: Vec<u8> = (0..len).map(|i| (i * 37 + 5) as u8).collect();
            let mut s = String::new();
            b64_encode(&bytes, &mut s);
            assert_eq!(b64_decode(&s).unwrap(), bytes);
        }
    }

    #[test]
    fn screens_arrive_whole_then_by_changes() {
        let mut f = Frame::new();
        f.clear(gfx::SKY);
        f.text(
            3,
            3,
            "Hello, Oregon! 1234567890",
            gfx::WHITE,
            gfx::Font::Normal,
        );
        for x in 0..W {
            f.pixel(x, 100, if x % 2 == 0 { gfx::RED } else { gfx::BLUE });
        }
        let mut tx = Sender::default();
        let mut rx = Screen::default();
        let send = |tx: &mut Sender, rx: &mut Screen, f: &Frame| {
            let lines = tx.lines(7, f);
            for l in &lines {
                assert!(l.len() < 2048, "line of {}", l.len());
                let w: Vec<&str> = l.split(' ').collect();
                rx.take(w[2].parse().unwrap(), w[3]);
            }
            lines.len()
        };
        assert!(send(&mut tx, &mut rx, &f) > 0);
        let mut out = Frame::new();
        rx.draw(&mut out);
        assert!(out.px == f.px, "the whole screen arrives");

        // Too soon: nothing. Then only the changed row.
        f.rect(10, 50, 5, 1, gfx::YELLOW);
        assert_eq!(send(&mut tx, &mut rx, &f), 0);
        tx.tick(FRAME_MS);
        let lines = tx.lines(7, &f);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].starts_with("scr 7 50 "));
        let w: Vec<&str> = lines[0].split(' ').collect();
        rx.take(50, w[3]);
        rx.draw(&mut out);
        assert!(out.px == f.px);
    }
}
