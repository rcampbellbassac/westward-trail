# Westward Trail

A kid-friendly wagon-trail game, inspired by the 1980s classic *The Oregon Trail*, for the browser and the M5Stack Cardputer
(original and Advanced). One Rust game crate draws every frame at the Cardputer's 240x135 resolution;
the web page scales it up, the Cardputer shows it as-is.

**Play it in your browser: https://rcampbellbassac.github.io/westward-trail/**

Made for a family's kids to play and tinker with. *The Oregon Trail* is a trademark of its owners; this
project isn't affiliated with them. The code, text and pixel art here are original, and the history
(landmarks, rivers, forts, the people in the starting Top Ten) is the real 1848 trail.

## What's in the game

- Choose who's playing: **younger pioneers** (gentle: nobody dies, sick folks get a ride home) or
  **seasoned pioneers** (the classic trail)
- Banker / carpenter / farmer, party names, leaving month, the general store
- 17 landmarks from Independence to the Willamette Valley, forts with stores, talking to travelers
- Weather, health, pace, rations, illnesses, injuries and ~18 kinds of random events
- River crossings: ford, caulk and float, ferry, hire a guide, wait
- Mini-games: **hunting** (move, aim, shoot; carry 100 lbs), **fishing** (timing), **Columbia River rafting**,
  and **gathering**: 28 real plants by place and month along the trail, including poisonous look-alikes
  (younger pioneers get a hint, then a warning; in classic mode the deadliest are rarely fatal)
- **Races, just for fun** (title screen: 6. Fun and games; on the trail: P): a **wagon race** (tap SPACE to hurry
  the oxen, at a stately 3 mph, and change lanes around rocks, bumps and trees), a **foot race** (jump the logs and
  rocks), and a **sled race** down a snowy hill when there's snow about. Nothing on the trip changes: `races.rs`.
- **Five save slots** (title screen: load; on the trail: S). Browser storage on the web; on the Cardputer,
  internal flash plus `WESTWARD/SAVE1.TXT`..`SAVE5.TXT` on the SD card (the newest copy wins at start-up; an edited save
  keeps its `saved = N` number, so a tie goes to the card and edits on a computer load)
- **Color-coded keys** for players who can't read yet: every menu number sits on its own colored key cap (1 blue,
  2 yellow, 3 pink...), ENTER and Y are green, ESC and N red, SPACE white. The on-screen keyboard (always on phones,
  **Show keys** with a mouse) uses the same colors, so "press the pink one" works. Every menu choice also has a
  little picture beside it (a wagon to travel, a fish to go fishing, a trophy for the Top Ten...): `icons.rs`.
- **Screenshots**: the Screenshot button on the web, Fn+P on the Cardputer (`WESTWARD/SHOT0001.BMP` on the SD card)
- SD cards can be **FAT32 or exFAT**. `firmware/components/fatfs` is ESP-IDF's FatFs component with exFAT switched on
  (`FF_FS_EXFAT 1` in `src/ffconf.h`); it replaces the built-in copy. Its files keep their own licenses.
- Trading, resting, the trail map, scoring with the occupation bonus, the Top Ten
- The secret **TRAIL-OS hacker terminal** (press `` ` ``) and a **mod editor** for the trail data

## Hosting it

GitHub Pages builds `site/` (landing page with the game built in, plus `play/`) on every push to `main`;
run `./site/build.sh` to build the same site locally in `dist/`.

To host it yourself: the web build is three static files (`web/index.html`, `main.js`, `trail.wasm`); any static web server works.
`./web/deploy.sh` rebuilds and rsyncs them to the server named in `web/deploy.env` (not committed):

```sh
DEPLOY_DEST=myserver:/srv/westward-trail/
DEPLOY_URL=https://trail.example.com/
```
## Layout

| Path | What it is |
|---|---|
| `crates/trail-core` | The whole game (`no_std`): scenes, simulation, mini-games, renderer |
| `crates/trail-core/data/trail.txt` | The mod file: landmarks, prices, animals, fish, illnesses, names |
| `crates/trail-core/src/sprites.rs` | Pixel art drawn with letters |
| `crates/trail-web` + `web/` | WebAssembly exports and the web page |
| `crates/trail-relay` | Home-network server for playing together: serves the web game and relays moves |
| `firmware/` | Cardputer firmware (ESP-IDF, Rust `esp` toolchain) |
| `docs/HACKING.md` | Guide for young hackers |

## Play in the browser

```sh
./web/build.sh
python3 -m http.server -d web 8080      # then open http://localhost:8080
```

High scores, the sound setting and any mod are saved in the browser.

## Cardputer

Uses the same toolchain and ESP-IDF install as `~/Source/CardputerPDA` (`firmware/.embuild` is a
symlink to that project's copy; see its `docs/SETUP.md` for a fresh machine).

```sh
cd firmware
source ~/export-esp.sh
cargo build --release                    # original Cardputer
cargo build --release --features adv     # Cardputer Advanced (TCA8418 keyboard, ES8311 sound)
espflash flash --monitor --port /dev/ttyACM0 target/xtensa-esp32s3-espidf/release/trail-firmware
```

Keys: numbers pick, `Enter` continues, `Fn` + `;` `.` `,` `/` are arrows (plain `;` `.` `,` `/` also
steer in the action games), `Space` shoots, `` ` `` opens the terminal, `Fn`+`` ` `` is Escape.

## Tests

```sh
cargo test -p trail-core            # parser, a robot that plays 40 trips, key-mashing fuzz
cargo run -p trail-core --example screens -- /tmp/shots   # PPM screenshots of every screen
```

## Status (2026-10-27)

- Core game, web build and both firmware builds compile with no warnings; tests pass.
- Web page verified in Node (all exports, mod loading, error messages); not yet clicked through in a browser.
- Flashed to both Cardputers on 2026-10-27. The Advanced build (keyboard mapping and ES8311 codec setup
  taken from M5Stack's M5Cardputer/M5Unified code) boots and plays; a full playthrough on each is still to do.
- **Playing together** (title menu 5): five games for up to 8 players, through `trail-relay` on a home computer or a
  Cardputer's own hotspot. The wagon race and wagon train give everyone their own wagon; one wagon with many
  drivers, democracy and hunting together share one game, whose screen the host sends to everyone. All five were
  played in browsers through the relay; Cardputer hosting is being tested. See `docs/MULTIPLAYER.md`.
- Ideas next: trail forks (Sublette Cutoff, Fort Walla Walla vs. The Dalles), tombstones with epitaphs
  saved for the next player, saving a trip in progress, high scores on the Cardputer's SD card.

## License

MIT or Apache-2.0, at your option (`LICENSE-MIT`, `LICENSE-APACHE`).
