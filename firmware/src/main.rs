//! Westward Trail on the M5Stack Cardputer.
//!
//! All of the game lives in `trail-core`. This file only connects it to the
//! hardware: keyboard in, 240x135 pixels out to the ST7789, beeps to the speaker,
//! saves to flash and the SD card.
//! Display, keyboard and speaker setup follow the tested CardputerPDA firmware.

mod keyboard;
mod storage;

use anyhow::Result;
use esp_idf_hal::{
    delay::{Ets, FreeRtos},
    gpio::{AnyInputPin, OutputPin as _, PinDriver},
    peripherals::Peripherals,
    spi::{config::Config as SpiConfig, SpiDeviceDriver, SpiDriver, SpiDriverConfig},
};
use esp_idf_svc::log::EspLogger;
use mipidsi::{
    interface::SpiInterface,
    models::ST7789,
    options::{ColorInversion, Orientation, Rotation},
    Builder,
};
use std::{
    sync::mpsc,
    time::{Duration, Instant},
};
use trail_core::{
    gfx::{color_from_raw, Frame, H, W},
    Beep, Game,
};

/// Aim for ~30 frames a second; the full-screen SPI transfer takes ~7 ms.
const FRAME: Duration = Duration::from_millis(33);

fn main() {
    esp_idf_svc::sys::link_patches();
    EspLogger::initialize_default();
    log::info!("Westward Trail starting");
    if let Err(error) = run() {
        log::error!("Westward Trail stopped: {error:#}");
        loop {
            FreeRtos::delay_ms(1_000);
        }
    }
}

fn run() -> Result<()> {
    let p = Peripherals::take()?;

    // Display: SCK 36, MOSI 35, CS 37, D/C 34, reset 33, backlight 38 (same on both models).
    let display_driver = SpiDriver::new(
        p.spi2,
        p.pins.gpio36,
        p.pins.gpio35,
        Option::<AnyInputPin>::None,
        &SpiDriverConfig::new(),
    )?;
    let spi = SpiDeviceDriver::new(
        display_driver,
        Some(p.pins.gpio37),
        &SpiConfig::new()
            .baudrate(80_000_000.into())
            .write_only(true),
    )?;
    let dc = PinDriver::output(p.pins.gpio34.downgrade_output())?;
    let reset = PinDriver::output(p.pins.gpio33.downgrade_output())?;
    let mut backlight = PinDriver::output(p.pins.gpio38)?;
    backlight.set_high()?;
    let mut display_buffer = [0_u8; 4096];
    let interface = SpiInterface::new(spi, dc, &mut display_buffer);
    let mut screen = Builder::new(ST7789, interface)
        .display_size(135, 240)
        .display_offset(52, 40)
        .orientation(Orientation::new().rotate(Rotation::Deg90))
        .invert_colors(ColorInversion::Inverted)
        .reset_pin(reset)
        .init(&mut Ets)
        .map_err(|e| anyhow::anyhow!("display init failed: {e:?}"))?;

    #[cfg(not(feature = "adv"))]
    let mut keys = keyboard::Keyboard::new(
        [
            p.pins.gpio8.into(),
            p.pins.gpio9.into(),
            p.pins.gpio11.into(),
        ],
        [
            p.pins.gpio13.into(),
            p.pins.gpio15.into(),
            p.pins.gpio3.into(),
            p.pins.gpio4.into(),
            p.pins.gpio5.into(),
            p.pins.gpio6.into(),
            p.pins.gpio7.into(),
        ],
    )?;
    // The Advanced's keyboard chip and ES8311 audio codec share I2C; this also wakes the codec.
    #[cfg(feature = "adv")]
    let mut keys = keyboard::Keyboard::new(p.i2c0, p.pins.gpio8, p.pins.gpio9)?;

    // Sound plays on its own thread so long tunes never freeze the game.
    let (beep_tx, beep_rx) = mpsc::channel::<Beep>();
    {
        let (i2s, bclk, data, ws) = (p.i2s1, p.pins.gpio41, p.pins.gpio42, p.pins.gpio43);
        std::thread::Builder::new()
            .name("trail-sound".into())
            .stack_size(6 * 1024)
            .spawn(move || {
                if let Err(e) = speaker::play_forever(i2s, bclk, data, ws, beep_rx) {
                    log::error!("speaker stopped: {e:#}");
                }
            })?;
    }
    // Saves and screenshots: internal flash, plus the SD card (SCK 40, MOSI 14, MISO 39, CS 12).
    let (event_tx, storage_events) = mpsc::channel::<storage::Event>();
    let (save_tx, save_rx) = mpsc::channel::<trail_core::SaveOp>();
    let sd = storage::SdPins {
        spi: p.spi3,
        sck: p.pins.gpio40,
        mosi: p.pins.gpio14,
        miso: p.pins.gpio39,
        cs: p.pins.gpio12,
    };
    std::thread::Builder::new()
        .name("trail-storage".into())
        .stack_size(16 * 1024)
        .spawn(move || storage::run(sd, event_tx, save_rx))?;
    let mut next_shot: Option<u32> = None;

    let seed = unsafe { esp_idf_svc::sys::esp_random() };
    let mut game = Game::new(u64::from(seed));
    let mut frame = Frame::new();
    let mut last = Instant::now();

    loop {
        let start = Instant::now();
        while let Ok(event) = storage_events.try_recv() {
            match event {
                storage::Event::Slot(i, text) => game.import_slot(i, &text),
                storage::Event::Scores(text) => game.import_scores(&text),
                storage::Event::SdReady(n) => next_shot = Some(n),
                storage::Event::Note(text) => game.notify(text),
            }
        }
        for event in keys.poll()? {
            match event {
                keyboard::Event::Down(k) => game.key_down(k),
                keyboard::Event::Up(k) => game.key_up(k),
                keyboard::Event::Screenshot => match next_shot {
                    // The frame still holds what's on screen right now.
                    Some(n) => match storage::screenshot(&frame, n) {
                        Ok(name) => {
                            next_shot = Some(n + 1);
                            game.notify(format!("Saved {name}"));
                        }
                        Err(e) => {
                            log::warn!("screenshot failed: {e:#}");
                            game.notify("Screenshot failed");
                        }
                    },
                    None => game.notify("Put in an SD card for screenshots"),
                },
            }
        }
        let now = Instant::now();
        game.tick(now.duration_since(last).as_millis() as u32);
        last = now;
        for b in game.take_beeps() {
            let _ = beep_tx.send(b);
        }
        for op in game.take_saves() {
            let _ = save_tx.send(op);
        }
        game.render(&mut frame);
        let pixels = frame.px.iter().map(|&raw| color_from_raw(raw));
        screen
            .set_pixels(0, 0, (W - 1) as u16, (H - 1) as u16, pixels)
            .map_err(|e| anyhow::anyhow!("display write failed: {e:?}"))?;
        if let Some(rest) = FRAME.checked_sub(start.elapsed()) {
            FreeRtos::delay_ms(rest.as_millis() as u32);
        } else {
            FreeRtos::delay_ms(1); // let the idle task feed the watchdog
        }
    }
}

mod speaker {
    //! Square-wave notes over I2S (41/42/43). The original drives an amplifier
    //! directly; the Advanced routes the same signal through its ES8311 codec.
    use anyhow::Result;
    use esp_idf_hal::{
        delay::FreeRtos,
        gpio::{AnyIOPin, Gpio41, Gpio42, Gpio43},
        i2s::{
            config::{DataBitWidth, StdConfig},
            I2sDriver, I2sTx, I2S1,
        },
    };
    use std::sync::mpsc::Receiver;
    use trail_core::Beep;

    const RATE: u32 = 16_000;
    const VOLUME: i16 = 1_800;

    pub fn play_forever(
        i2s: I2S1,
        bclk: Gpio41,
        data: Gpio42,
        ws: Gpio43,
        rx: Receiver<Beep>,
    ) -> Result<()> {
        let config = StdConfig::philips(RATE, DataBitWidth::Bits16);
        let mut i2s =
            I2sDriver::<I2sTx>::new_std_tx(i2s, &config, bclk, data, Option::<AnyIOPin>::None, ws)?;
        let mut buf = [0_u8; 1024];
        while let Ok(first) = rx.recv() {
            i2s.tx_enable()?;
            let mut next = Some(first);
            while let Some(b) = next {
                let mut frames = RATE * u32::from(b.ms) / 1000;
                let half = if b.hz == 0 {
                    u32::MAX
                } else {
                    (RATE / 2 / u32::from(b.hz)).max(1)
                };
                let mut phase = 0_u32;
                while frames > 0 {
                    let n = frames.min((buf.len() / 4) as u32);
                    for i in 0..n as usize {
                        let s: i16 = if b.hz == 0 {
                            0
                        } else if (phase / half) % 2 == 0 {
                            VOLUME
                        } else {
                            -VOLUME
                        };
                        phase += 1;
                        let s = s.to_le_bytes();
                        buf[i * 4..i * 4 + 2].copy_from_slice(&s);
                        buf[i * 4 + 2..i * 4 + 4].copy_from_slice(&s);
                    }
                    i2s.write_all(&buf[..n as usize * 4], 100)?;
                    frames -= n;
                }
                next = rx.try_recv().ok();
            }
            // Flush with silence, then stop the clock so DMA can't replay the last note.
            buf.fill(0);
            i2s.write_all(&buf, 100)?;
            FreeRtos::delay_ms(20);
            i2s.tx_disable()?;
        }
        Ok(())
    }
}
