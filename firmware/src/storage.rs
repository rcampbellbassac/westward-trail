//! Saved trips, the Top Ten and screenshots.
//!
//! Saves always go to the internal flash (NVS), and also to the SD card when
//! one is in: `WESTWARD/SAVE1.TXT` .. `SAVE5.TXT` and `SCORES.TXT`. At start-up
//! the newest copy of each slot wins (by its `saved = N` counter, since there's
//! no clock) and is copied to the other place. On a tie the SD copy wins, so a
//! save edited on a computer loads. The two Top Tens are merged.
//! Screenshots go to the SD card only, as `WESTWARD/SHOT0001.BMP` and up.
//!
//! The SD card is opened on its own thread like in CardputerPDA: some cards (and
//! an empty slot) make the driver wait a long time, and the game shouldn't freeze.

use std::{
    fs,
    io::Write,
    sync::mpsc::{Receiver, Sender},
};

use anyhow::Result;
use esp_idf_hal::{
    gpio::{AnyIOPin, Gpio12, Gpio14, Gpio39, Gpio40},
    sd::{spi::SdSpiHostDriver, SdCardConfiguration, SdCardDriver},
    spi::{Dma, SpiDriver, SpiDriverConfig, SPI3},
};
use esp_idf_svc::{
    fs::fatfs::Fatfs,
    io::vfs::MountedFatfs,
    nvs::{EspDefaultNvsPartition, EspNvs, NvsDefault},
};
use trail_core::{gfx::Frame, save_sequence, SaveOp, SLOTS};

const DIR: &str = "/sdcard/WESTWARD";

/// From the storage thread to the game.
pub enum Event {
    Slot(usize, String),
    Scores(String),
    /// The SD card is ready; screenshots can go to it, starting at this number.
    SdReady(u32),
    Note(String),
}

pub struct SdPins {
    pub spi: SPI3,
    pub sck: Gpio40,
    pub mosi: Gpio14,
    pub miso: Gpio39,
    pub cs: Gpio12,
}

fn sd_name(op: &SaveOp) -> String {
    match op {
        SaveOp::Slot(i, _) => format!("{DIR}/SAVE{}.TXT", i + 1),
        SaveOp::Scores(_) => format!("{DIR}/SCORES.TXT"),
    }
}

fn nvs_key(op: &SaveOp) -> String {
    match op {
        SaveOp::Slot(i, _) => format!("slot{i}"),
        SaveOp::Scores(_) => String::from("scores"),
    }
}

fn text(op: &SaveOp) -> &str {
    match op {
        SaveOp::Slot(_, t) | SaveOp::Scores(t) => t,
    }
}

fn to_event(op: SaveOp) -> Event {
    match op {
        SaveOp::Slot(i, t) => Event::Slot(i, t),
        SaveOp::Scores(t) => Event::Scores(t),
    }
}

/// Everything there is to load: five slots and the scores.
fn all_ops() -> impl Iterator<Item = SaveOp> {
    (0..SLOTS)
        .map(|i| SaveOp::Slot(i, String::new()))
        .chain(std::iter::once(SaveOp::Scores(String::new())))
}

fn with_text(op: &SaveOp, t: String) -> SaveOp {
    match op {
        SaveOp::Slot(i, _) => SaveOp::Slot(*i, t),
        SaveOp::Scores(_) => SaveOp::Scores(t),
    }
}

fn open_nvs() -> Result<EspNvs<NvsDefault>> {
    Ok(EspNvs::new(EspDefaultNvsPartition::take()?, "trail", true)?)
}

fn open_sd(pins: SdPins) -> Result<Fatfs<SdCardDriver<SdSpiHostDriver<'static, SpiDriver<'static>>>>> {
    let spi = SpiDriver::new(
        pins.spi,
        pins.sck,
        pins.mosi,
        Some(pins.miso),
        &SpiDriverConfig::new().dma(Dma::Auto(4096)),
    )?;
    let host = SdSpiHostDriver::new(
        spi,
        Some(pins.cs),
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        Option::<AnyIOPin>::None,
        None,
    )?;
    let card = SdCardDriver::new_spi(host, &SdCardConfiguration::new())?;
    Ok(Fatfs::new_sdcard(0, card)?)
}

/// The number after the highest SHOTnnnn.BMP already on the card.
fn next_shot() -> u32 {
    let Ok(dir) = fs::read_dir(DIR) else {
        return 1;
    };
    dir.flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_uppercase();
            name.strip_prefix("SHOT")?
                .strip_suffix(".BMP")?
                .parse::<u32>()
                .ok()
        })
        .max()
        .map_or(1, |n| n + 1)
}

/// Runs forever on its own thread: loads saves, then writes whatever the game sends.
pub fn run(pins: SdPins, events: Sender<Event>, commands: Receiver<SaveOp>) {
    let mut nvs = match open_nvs() {
        Ok(nvs) => Some(nvs),
        Err(e) => {
            log::warn!("internal save storage unavailable: {e:#}");
            None
        }
    };
    // Internal saves first: they're quick, and there may be no card at all.
    let mut inside: Vec<Option<String>> = Vec::new();
    {
        let mut buf = vec![0u8; 4000];
        for op in all_ops() {
            let t = nvs
                .as_ref()
                .and_then(|nvs| nvs.get_str(&nvs_key(&op), &mut buf).ok().flatten())
                .map(str::to_string);
            if let Some(t) = &t {
                let _ = events.send(to_event(with_text(&op, t.clone())));
            }
            inside.push(t);
        }
    }

    let sd = open_sd(pins).and_then(|fatfs| Ok(MountedFatfs::mount(fatfs, "/sdcard", 4)?));
    let mount = match sd {
        Ok(mount) => match fs::create_dir_all(DIR) {
            Ok(()) => Some(mount),
            Err(e) => {
                log::warn!("SD card: can't make {DIR}: {e}");
                None
            }
        },
        Err(e) => {
            log::info!("no SD card: {e:#}");
            None
        }
    };
    if mount.is_some() {
        for (op, inside) in all_ops().zip(inside) {
            let card = fs::read_to_string(sd_name(&op)).ok();
            let best = match (&op, inside, card) {
                (_, None, None) => continue,
                (SaveOp::Scores(_), Some(a), Some(b)) => merge_scores(&a, &b),
                (_, Some(a), Some(b)) if save_sequence(&a) > save_sequence(&b) => a,
                (_, _, Some(b)) => b,
                (_, Some(a), None) => a,
            };
            let op = with_text(&op, best);
            // Make both places match.
            if let Some(nvs) = nvs.as_mut() {
                let _ = nvs.set_str(&nvs_key(&op), text(&op));
            }
            let _ = fs::write(sd_name(&op), text(&op));
            let _ = events.send(to_event(op));
        }
        let _ = events.send(Event::SdReady(next_shot()));
    }

    while let Ok(op) = commands.recv() {
        if let Some(nvs) = nvs.as_mut() {
            if let Err(e) = nvs.set_str(&nvs_key(&op), text(&op)) {
                log::warn!("internal save failed: {e:#}");
                let _ = events.send(Event::Note(String::from("Couldn't save inside!")));
            }
        }
        if mount.is_some() {
            if let Err(e) = fs::write(sd_name(&op), text(&op)) {
                log::warn!("SD save failed: {e}");
                let _ = events.send(Event::Note(String::from("SD card save failed")));
            } else if matches!(op, SaveOp::Slot(..)) {
                let _ = events.send(Event::Note(String::from("Saved (and on the SD card)")));
            }
        }
    }
}

/// Both Top Tens together: the best ten entries, without repeats.
fn merge_scores(a: &str, b: &str) -> String {
    let mut all: Vec<(u32, &str)> = a
        .lines()
        .chain(b.lines())
        .filter_map(|l| Some((l.rsplit_once('|')?.1.trim().parse().ok()?, l.trim())))
        .collect();
    all.sort_by(|x, y| y.0.cmp(&x.0).then(x.1.cmp(y.1)));
    all.dedup_by(|x, y| x.1 == y.1);
    all.iter().take(10).map(|(_, l)| format!("{l}\n")).collect()
}

/// Write a screenshot straight from the frame. Returns the file name.
pub fn screenshot(frame: &Frame, number: u32) -> Result<String> {
    let name = format!("SHOT{number:04}.BMP");
    let mut file = std::io::BufWriter::new(fs::File::create(format!("{DIR}/{name}"))?);
    let mut result = Ok(());
    frame.write_bmp(|chunk| {
        if result.is_ok() {
            result = file.write_all(chunk);
        }
    });
    result?;
    file.flush()?;
    Ok(name)
}
