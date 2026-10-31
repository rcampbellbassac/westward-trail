# Multiplayer plan

Decided 2026-10-27. Work happens straight on `main`, behind the title menu's "Play together", so the
single-player game stays exactly as it is.

## Status

| Milestone | State |
|---|---|
| 1. Core: lobby, shared seed, race screens | **Done.** Tested with three games talking through a pretend relay. |
| 2. `trail-relay` on a home computer | **Done** and running on our home server. Two browsers raced through it. |
| 3. Cardputer host (hotspot, serves the web game, relays) | **Built, testing on hardware.** The host screen now counts devices on the Wi-Fi, pages loaded and players, to show where joining gets stuck. |
| 4. Cardputers as players (join a hotspot or the home relay) | **Built, not yet working** on hardware: joining a Cardputer hotspot timed out (being looked into). |
| 5. Wagon train | **Done.** Giving supplies between nearby wagons; messages from the list. |
| 6. Democracy | **Done.** Played in two browsers. |
| 7. One wagon, many drivers | **Done.** Played in two browsers. |
| 8. Hunt together | **Done**, as its own game and inside the one-wagon games. Played in two browsers. |
| 9. The standalone relay for other families | Not started. |
| Fun races (wagon, foot and sled races, up to 4 players) | **Done.** Raced in two browsers. |

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
| 6 | **Fun races** | The wagon, foot and sled races from "Fun and games", up to 4 players. Computer racers fill the empty spots. |

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

## How the one-party games work

In the wagon race and wagon train everyone runs their own trip, and the shared seed keeps their luck the same. In
one wagon with many drivers, democracy, and hunting together there is only one trip, so only the host runs it. The
host sends its **screen** to everyone else as text lines, and they send their **keys** back (`party.rs`,
`stream.rs`):

- Only rows that changed are sent, run-length coded and in base64, at most about 12 times a second, plus the
  whole screen every 2.5 seconds for anyone who missed some. A menu that isn't changing sends nothing.
- Watchers' keys arrive at the host as `key` lines. Who may press what:
  - **One wagon:** only the driver. The turn passes to the next player each time the wagon sets off.
  - **Democracy:** at a choice (size-up menu, pace, rations, rivers, trades, the Dalles, who to be) every number
    is a vote. The vote ends when everyone has voted, when the timer runs out, or with no timer when the leader
    presses ENTER. On a tie the leader's vote wins. The leader types names and shops, and anyone can turn the page
    of a message. A player whose party member is gone is a ghost and can't vote.
  - **Hunting:** everyone has their own hunter, in their own color with their initial over it.
  - **Fun races:** everyone has their own racer in their own color. The host picks the race. The screen
    follows the middle of the players, and anyone more than a screen behind is pulled along to its edge, so
    nobody gets lost off the shared screen.
- The party members are the players, in the order they joined (the host leads).
- A watcher presses ` (backtick) for their own menu: send a message or leave.
- Lines are now up to 2048 characters (the relay and the Cardputer were raised from 512).

## How the code changes

- The game core stays `no_std` and knows nothing about networks, just as it knows nothing about speakers:
  `game.take_net() -> Vec<NetMsg>` to send and `game.net_in(msg)` for incoming messages, shaped like the beep queue.
- Messages are short text lines (`pos sam 830 day=171`), easy to debug and hackable like the save files.
- With a shared seed, the world (weather, river depths) agrees without being sent. Only actions, votes and positions
  travel.
- The relay is a new workspace crate (`trail-relay`) that builds for Linux (the home relay, and later the standalone
  server). The Cardputer host reuses the same message rules, with `esp-idf-svc`'s HTTP and WebSocket server.

## Milestones

1. Core: lobby, shared seed, race screens, tested by running two games side by side in unit tests.
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
