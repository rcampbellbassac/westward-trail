# Multiplayer plan

Decided 2026-10-27. Work happens straight on `main`, behind the title menu's "Play together", so the
single-player game stays exactly as it is.

## Status

| Milestone | State |
|---|---|
| 1. Core: lobby, shared seed, race screens | **Done.** Tested with three games talking through a pretend relay. |
| 2. `trail-relay` on a home computer | **Done** and running on our home server. Two browsers raced through it. |
| 3. Cardputer host (hotspot, serves the web game, relays) | **Built, testing on hardware.** |
| 4. Cardputers as players (join a hotspot or the home relay) | **Built, not yet tested** (needs a second device). |
| 5-9 | Not started. The host menu lists the other modes as "coming soon". |

### Try it

- **Browsers at home:** `./web/build.sh`, then `cargo run --release -p trail-relay -- --web web` on a computer on
  the home network. Everyone opens `http://<that computer>:8850`, picks **5. Play together**, one player hosts and
  the others join. Cardputers on the home Wi-Fi pick **2. Join a game on Wi-Fi**, type the home network, and type the
  relay computer's address once (it's remembered). On the same network segment they can leave it blank and it's found
  automatically, but broadcasts don't cross VLANs.
- **Cardputer host:** on the Cardputer, **5. Play together** then **1. Host a game on this Cardputer**. Phones and
  computers join the Wi-Fi it shows (or scan the QR code) and open `http://192.168.4.1`. Another Cardputer picks
  **2. Join a game on Wi-Fi** and types the same name and password.
- The firmware embeds the web game, so run `./web/build.sh` before building the firmware.

## Goals

- Kids play **together in the same room**: Cardputers, browsers on a Chromebook or tablet, or a mix.
- **No internet needed and nothing leaves the group.** No accounts, no public server.
- **Up to 8 players.**

## Modes (built in this order)

| # | Mode | How it plays |
|---|---|---|
| 1 | **Wagon race** | Same trail, same seed (same weather and luck). Everyone drives their own wagon and sees the others on the map and travel screen ("Sam reached Fort Laramie!"). Shared Top Ten at the end. |
| 2 | **Wagon train** | The race, plus trading and giving supplies between wagons, and messages picked from a list. |
| 3 | **Hunt together** | Several hunters in the same field in real time, sharing the meat. |
| 4 | **One wagon, many drivers** | Players take turns making the choices for one shared party. |
| 5 | **Democracy** | Everyone is one member of the same party, and the party votes on every choice. |

### Democracy rules

- Each player *is* a party member; the wagon leader is party member #1.
- Every choice (pace, rations, ford or ferry, trades...) is a vote. **Ties: the wagon leader decides.**
- **The host picks the vote timer: 10 s, 20 s, 30 s, or no timer** (with no timer, the leader can close the vote).
  Players who haven't voted when time runs out abstain, so any number of players works.
- The vote ends early once everyone has voted.
- If your party member gets sick, you still vote. If they leave the party (gentle) or die (classic), you become a ghost who can
  watch and cheer but not vote.

### Messages

Picked from a list only ("Need food!", "Race you to Oregon!", "Vote yes!", "Wait for me!"). Fast on a tiny keyboard,
and nothing unkind can be typed. The list lives in the trail file, so it's moddable.

### Cheats

**The host decides** for the whole game: cheats on (with `[H]` shown next to the cheater's name for everyone) or off.

## Connecting: both ways

**1. A Cardputer hosts (anywhere).**
One Cardputer becomes a Wi-Fi hotspot named `WestwardTrail-XXXX` with a **new random password each time**, shown on
the host's screen along with a QR code for phones. It serves the web game itself at `http://192.168.4.1` (the 270 KB
`trail.wasm` fits easily in flash), so phones, tablets and Chromebooks join by opening a page, with nothing to
install. It relays game messages over a WebSocket. Other Cardputers join the same hotspot.

**2. Home network relay (at home).**
A small relay program on a home computer (a Raspberry Pi is plenty), reachable on the home network only, with no sign-in so Cardputers and the kids'
devices can join. Like the hotspot, it serves the web game itself as well as relaying.

Later, the relay becomes a **standalone server anyone can run** for their own family (a single program plus
instructions). We don't host it for other people.

Why both serve the game themselves: the public GitHub Pages site is HTTPS, and browsers block an HTTPS page from
talking to a plain `ws://` address on a home network. Loading the game from the hotspot or relay avoids that.

## How the code changes

- The game core stays `no_std` and knows nothing about networks, just as it knows nothing about speakers:
  `game.take_net() -> Vec<NetMsg>` to send and `game.net_in(msg)` for incoming messages, shaped like the beep queue.
- Messages are short text lines (`pos sam 830 day=171`), easy to debug and hackable like the save files.
- With a shared seed, the world (weather, river depths) agrees without being sent. Only actions, votes and positions
  travel.
- The relay is a new workspace crate (`trail-relay`) that builds for Linux (the home relay, and later the standalone
  server). The Cardputer host reuses the same message rules, with `esp-idf-svc`'s HTTP and WebSocket server.

## Milestones

1. Core: lobby, shared seed, `NetMsg`, race screens, tested by running two games side by side in unit tests.
2. `trail-relay` on a home computer (home network only) serving the web game; browsers race each other.
3. Cardputer host: hotspot with random password and QR code, serves the web game, relays.
4. Cardputers as players (join a hotspot or the home relay).
5. Wagon train: trading and list messages.
6. Democracy mode (with the host's timer choice).
7. One wagon, many drivers.
8. Hunt together (real time).
9. The standalone relay, packaged with instructions for other families.

## Safety

- The hotspot has a fresh password every time and nothing reaches the internet.
- Messages come from a fixed list, never free typing.
- The home relay only listens on the home network.
