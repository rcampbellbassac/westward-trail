# Hacking the Trail

Hi, young hacker! This game is built so you can change it. Here's where to start, from easiest to hardest.

## 1. The secret terminal

Press the `` ` `` key (top left of the keyboard) anywhere outside the action games. Type `help`.
Try `give food 9999`, `theme green`, `weather snow`, `matrix`, `whoami`. Press ESC to leave.
Using cheats puts **[H]** next to your name in the Top Ten. Everyone will know you're a hacker.

## 2. The mod file

`crates/trail-core/data/trail.txt` holds the whole trail. In the web version, press **Mod editor**,
change something, press **Apply**. Ideas:

- Make buffalo weigh 5000 pounds (`max_lbs = 5000` under `[animal]` buffalo).
- Make oxen cost a penny (`oxen = 0.01` under `[store]`).
- Add your own landmark between two others: copy a `[landmark]` block and change the name and mile.
- Add an animal. It needs `name`, `min_lbs`, `max_lbs`, and a `sprite` (squirrel, rabbit, deer, buffalo, bear, elk).
- Add a new sickness, like `name = the giggles`.

If you make a mistake, the game tells you the line number. Fix it and press Apply again.

Plants you can gather are `[plant]` sections. `miles = 0-640` is where it grows and `months = 5-7` is
when it's ripe (May to July). Make strawberries grow all year, or invent a candy bush.

## 2½. Hack a saved game

Saved trips are plain text too. On the Cardputer with an SD card, they're `WESTWARD/SAVE1.TXT` to
`SAVE5.TXT`. Open one on a computer, change `food = 300` to `food = 9999`, put the card back, and load
the trip. Leave the `saved = ...` line alone: it tells the Cardputer which copy is newest. (In the browser they live in the page's storage, so the terminal is easier there.)

## 3. Redraw the pictures

`crates/trail-core/src/sprites.rs` has the pictures drawn with letters: `b` is brown, `w` white,
`.` is see-through. Change some letters, then run `./web/build.sh` and reload the page.

## 4. Add a terminal command

Open `crates/trail-core/src/terminal.rs` and find `fn run`. Every command is one line like:

```rust
"hello" | "hi" => out(t, "Hello! Type help."),
```

Copy it, change the word and the message, rebuild. Then make one that *does* something, like
`g.t.sup.food += 100;`.

## 5. Change the rules

- How fast you travel: `miles_today` in `travel.rs`
- How people get sick: `illness` in `travel.rs`
- Random events (thieves, broken wheels, wild berries): `random_event` in `travel.rs`
- Hunting time limit and how much you can carry: the top of `hunt.rs`
- Gathering time, basket size and what poisonous plants do: `forage.rs`

Run `cargo test -p trail-core` after big changes. A robot plays 40 whole trips to check nothing broke, and pretend
players try every way of playing together.

To look at screens without a device: `TRAIL_SHOTS=/some/folder cargo test -p trail-core shots` saves the map and
travel screens there as BMP pictures. Add more scenes to the `shots` test in `lib.rs`.
