//! Cardputer keyboard → game keys.
//!
//! Both models share the same 4x14 key layout. The original scans a GPIO matrix
//! (code from CardputerPDA, tested); the Advanced reads a TCA8418 over I2C
//! (`--features adv`, mapping taken from M5Stack's M5Cardputer library).
//!
//! Fn + ; . , /  are the arrow keys printed on the keycaps, Fn + ` is Escape.
//! The game also treats plain ; . , / as arrows in hunting and rafting.

use anyhow::Result;
use trail_core::Key;

pub enum Event {
    Down(Key),
    Up(Key),
}

const NORMAL: [&str; 4] = [
    "`1234567890-=\u{8}",
    "\tqwertyuiop[]\\",
    "\0\0asdfghjkl;'\n",
    "\0\0\0zxcvbnm,./ ",
];
const UPPER: [&str; 4] = [
    "~!@#$%^&*()_+\u{8}",
    "\tQWERTYUIOP{}|",
    "\0\0ASDFGHJKL:\"\n",
    "\0\0\0ZXCVBNM<>? ",
];
const FN_POS: (usize, usize) = (0, 2);
const SHIFT_POS: (usize, usize) = (1, 2);

#[cfg(not(feature = "adv"))]
/// Matrix (selection, input) → visible (x, y) position on the keyboard.
fn key_position(selection: u8, input: u8) -> (usize, usize) {
    let x = usize::from(input) * 2 + usize::from(selection <= 3);
    let y = 3 - usize::from(selection % 4);
    (x, y)
}

fn is_modifier(pos: (usize, usize)) -> bool {
    matches!(pos, (0, 2) | (1, 2) | (0, 3) | (1, 3) | (2, 3))
}

fn decode(pos: (usize, usize), shift: bool, fn_held: bool) -> Option<Key> {
    let table = if shift { UPPER } else { NORMAL };
    let c = table[pos.1].chars().nth(pos.0)?;
    Some(match (fn_held, c) {
        (true, ';') => Key::Up,
        (true, '.') => Key::Down,
        (true, ',') => Key::Left,
        (true, '/') => Key::Right,
        (true, '`') => Key::Escape,
        (_, '\u{8}') => Key::Backspace,
        (_, '\n') => Key::Enter,
        (_, '\0' | '\t') => return None,
        (_, c) => Key::Char(c),
    })
}

/// Turns "which positions are held right now" into key down/up events.
#[derive(Default)]
struct Tracker {
    held: Vec<((usize, usize), Key)>,
}

impl Tracker {
    fn update(&mut self, now: &[(usize, usize)]) -> Vec<Event> {
        let mut events = Vec::new();
        self.held.retain(|(pos, key)| {
            let still = now.contains(pos);
            if !still {
                events.push(Event::Up(*key));
            }
            still
        });
        let shift = now.contains(&SHIFT_POS);
        let fn_held = now.contains(&FN_POS);
        for &pos in now {
            if is_modifier(pos) || self.held.iter().any(|(p, _)| *p == pos) {
                continue;
            }
            if let Some(key) = decode(pos, shift, fn_held) {
                self.held.push((pos, key));
                events.push(Event::Down(key));
            }
        }
        events
    }
}

#[cfg(not(feature = "adv"))]
pub use original::Keyboard;

#[cfg(not(feature = "adv"))]
mod original {
    use super::*;
    use esp_idf_hal::{
        delay::Ets,
        gpio::{AnyIOPin, AnyOutputPin, Input, Output, PinDriver, Pull},
    };

    /// Direct GPIO matrix scanner for the original Cardputer.
    pub struct Keyboard<'d> {
        select: [PinDriver<'d, AnyOutputPin, Output>; 3],
        sense: [PinDriver<'d, AnyIOPin, Input>; 7],
        tracker: Tracker,
    }

    impl<'d> Keyboard<'d> {
        pub fn new(select: [AnyOutputPin; 3], sense: [AnyIOPin; 7]) -> Result<Self> {
            let [s0, s1, s2] = select;
            let mut select = [
                PinDriver::output(s0)?,
                PinDriver::output(s1)?,
                PinDriver::output(s2)?,
            ];
            let [i0, i1, i2, i3, i4, i5, i6] = sense;
            let mut sense = [
                PinDriver::input(i0)?,
                PinDriver::input(i1)?,
                PinDriver::input(i2)?,
                PinDriver::input(i3)?,
                PinDriver::input(i4)?,
                PinDriver::input(i5)?,
                PinDriver::input(i6)?,
            ];
            for pin in &mut sense {
                pin.set_pull(Pull::Up)?;
            }
            for pin in &mut select {
                pin.set_low()?;
            }
            Ok(Self {
                select,
                sense,
                tracker: Tracker::default(),
            })
        }

        pub fn poll(&mut self) -> Result<Vec<Event>> {
            let mut now = Vec::new();
            for selection in 0_u8..8 {
                for (bit, pin) in self.select.iter_mut().enumerate() {
                    pin.set_level(((selection & (1 << bit)) != 0).into())?;
                }
                Ets::delay_us(50);
                for (input, pin) in self.sense.iter().enumerate() {
                    if pin.is_low() {
                        now.push(key_position(selection, input as u8));
                    }
                }
            }
            Ok(self.tracker.update(&now))
        }
    }
}

#[cfg(feature = "adv")]
pub use advanced::Keyboard;

#[cfg(feature = "adv")]
mod advanced {
    //! TCA8418 keypad controller on the Cardputer Advanced: I2C SDA 8 / SCL 9
    //! (M5Unified pin table), address 0x34, a 7 row x 8 column matrix (M5Cardputer
    //! TCA8418KeyboardReader). The ES8311 audio codec sits on the same bus.
    use super::*;
    use esp_idf_hal::{
        delay::BLOCK,
        gpio::{Gpio8, Gpio9},
        i2c::{I2cConfig, I2cDriver, I2C0},
        units::FromValueType,
    };

    const ADDR: u8 = 0x34;
    const REG_CFG: u8 = 0x01;
    const REG_INT_STAT: u8 = 0x02;
    const REG_KEY_LCK_EC: u8 = 0x03;
    const REG_KEY_EVENT_A: u8 = 0x04;
    const REG_KP_GPIO1: u8 = 0x1D;
    const REG_KP_GPIO2: u8 = 0x1E;
    const REG_KP_GPIO3: u8 = 0x1F;

    const ES8311_ADDR: u8 = 0x18;
    /// Codec power-up, copied from M5Unified's Cardputer ADV speaker callback:
    /// clock from BCLK x8, analog and DAC on, headphone driver on, 0 dB.
    const ES8311_ON: [(u8, u8); 8] = [
        (0x00, 0x80),
        (0x01, 0xB5),
        (0x02, 0x18),
        (0x0D, 0x01),
        (0x12, 0x00),
        (0x13, 0x10),
        (0x32, 0xBF),
        (0x37, 0x08),
    ];

    /// TCA8418 (row, column) -> visible (x, y), as M5Cardputer's remap().
    fn tca_position(row: u8, col: u8) -> (usize, usize) {
        (usize::from(row) * 2 + usize::from(col > 3), usize::from(col % 4))
    }

    pub struct Keyboard<'d> {
        i2c: I2cDriver<'d>,
        down: Vec<(usize, usize)>,
        tracker: Tracker,
    }

    impl<'d> Keyboard<'d> {
        pub fn new(i2c: I2C0, sda: Gpio8, scl: Gpio9) -> Result<Self> {
            let config = I2cConfig::new().baudrate(400.kHz().into());
            let mut i2c = I2cDriver::new(i2c, sda, scl, &config)?;
            // Rows 0-6 and columns 0-7 are keypad lines; enable the key-event FIFO.
            for (reg, val) in [
                (REG_KP_GPIO1, 0x7F),
                (REG_KP_GPIO2, 0xFF),
                (REG_KP_GPIO3, 0x00),
                (REG_CFG, 0x01),
            ] {
                i2c.write(ADDR, &[reg, val], BLOCK)?;
            }
            // Sound is nice to have: a codec problem shouldn't stop the game.
            for (reg, val) in ES8311_ON {
                if let Err(e) = i2c.write(ES8311_ADDR, &[reg, val], BLOCK) {
                    log::warn!("ES8311 codec setup failed: {e:?}");
                    break;
                }
            }
            Ok(Self {
                i2c,
                down: Vec::new(),
                tracker: Tracker::default(),
            })
        }

        pub fn poll(&mut self) -> Result<Vec<Event>> {
            let mut count = [0_u8];
            self.i2c
                .write_read(ADDR, &[REG_KEY_LCK_EC], &mut count, BLOCK)?;
            for _ in 0..(count[0] & 0x0F) {
                let mut ev = [0_u8];
                self.i2c
                    .write_read(ADDR, &[REG_KEY_EVENT_A], &mut ev, BLOCK)?;
                let number = ev[0] & 0x7F;
                if number == 0 || number > 80 {
                    continue;
                }
                let (row, col) = ((number - 1) / 10, (number - 1) % 10);
                if row > 6 || col > 7 {
                    log::info!("unmapped TCA8418 key row {row} col {col}");
                    continue;
                }
                let pos = tca_position(row, col);
                if ev[0] & 0x80 != 0 {
                    if !self.down.contains(&pos) {
                        self.down.push(pos);
                    }
                } else {
                    self.down.retain(|p| *p != pos);
                }
            }
            if count[0] & 0x0F != 0 {
                self.i2c.write(ADDR, &[REG_INT_STAT, 0x01], BLOCK)?;
            }
            Ok(self.tracker.update(&self.down))
        }
    }
}
