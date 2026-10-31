//! Little pictures beside menu choices, for players who can't read yet.
//!
//! A menu line like "3. See the Top Ten" is drawn with a picture in place of the ". ",
//! so nothing moves on the screen. The picture is picked by the words in the line
//! ([`for_label`]); the first match in [`PICKS`] wins. Letters work as in `sprites.rs`.

use crate::sprites::Sprite;

const fn icon(rows: &'static [&'static str]) -> Sprite {
    Sprite { rows }
}

pub static WAGON: Sprite = icon(&[
    "...wwwww..",
    "..wwwwwww.",
    "..wwwwwww.",
    ".bbbbbbbbb",
    ".bbbbbbbbb",
    "..kk...kk.",
    ".kaak.kaak",
    "..kk...kk.",
]);
pub static QUESTION: Sprite = icon(&[
    "...yyyy...",
    "..yy..yy..",
    "......yy..",
    ".....yy...",
    "....yy....",
    "....yy....",
    "..........",
    "....yy....",
    "....yy....",
]);
pub static TROPHY: Sprite = icon(&[
    ".yyyyyyyy.",
    "yyyyyyyyyy",
    "y.yyyyyy.y",
    ".y.yyyy.y.",
    "...yyyy...",
    "....yy....",
    "....yy....",
    "..oooooo..",
    "..oooooo..",
]);
pub static CHEST: Sprite = icon(&[
    ".bbbbbbbb.",
    "bBBBBBBBBb",
    "bbbbbbbbbb",
    "bBBByyBBBb",
    "bbbbyybbbb",
    "bBBBBBBBBb",
    "bBBBBBBBBb",
    "bbbbbbbbbb",
]);
pub static PEOPLE: Sprite = icon(&[
    ".ss....ss.",
    ".ss....ss.",
    "..........",
    "uuuu..rrrr",
    "uuuu..rrrr",
    ".uu....rr.",
    ".uu....rr.",
    ".u.u..r.r.",
    ".u.u..r.r.",
]);
pub static HEART: Sprite = icon(&[
    ".rr...rr..",
    "rrrr.rrrr.",
    "rrrrrrrrr.",
    "rrrrrrrrr.",
    ".rrrrrrr..",
    "..rrrrr...",
    "...rrr....",
    "....r.....",
]);
pub static BOLT: Sprite = icon(&[
    ".....yyy..",
    "....yyy...",
    "...yyy....",
    "..yyyyyy..",
    "....yyy...",
    "...yyy....",
    "..yyy.....",
    "..yy......",
    "..y.......",
]);
pub static MONEY: Sprite = icon(&[
    "...BBBB...",
    "....BB....",
    "..tttttt..",
    ".tttgggtt.",
    "tttgtttttt",
    "ttttgggttt",
    "ttttttgttt",
    ".ttgggttt.",
    "..tttttt..",
]);
pub static HAMMER: Sprite = icon(&[
    ".aaaaaa...",
    "aaaaaaaa..",
    ".aaaaaa...",
    "...bb.....",
    "...bb.....",
    "...bb.....",
    "...bb.....",
    "...bb.....",
    "...bb.....",
]);
pub static WHEAT: Sprite = icon(&[
    "....y.....",
    "...yyy....",
    "...yyy....",
    ".g.yyy.g..",
    "..gyyyg...",
    "...gyg....",
    "....g.....",
    "....g.....",
    "....g.....",
]);
pub static SNOW: Sprite = icon(&[
    "....w.....",
    ".w..w..w..",
    "..w.w.w...",
    "...www....",
    "wwwwwwwww.",
    "...www....",
    "..w.w.w...",
    ".w..w..w..",
    "....w.....",
]);
pub static RAIN: Sprite = icon(&[
    "...aaa....",
    ".aaaaaaa..",
    "aaaaaaaaa.",
    ".aaaaaaa..",
    "..........",
    ".u..u..u..",
    "u..u..u...",
    "..........",
    ".u..u..u..",
]);
pub static FLOWER: Sprite = icon(&[
    "...pp.....",
    "..pyyp....",
    "..pyyp....",
    "...pp.....",
    "....g.....",
    ".gg.g.....",
    "..ggg.....",
    "....g.....",
    "....g.....",
]);
pub static SUN: Sprite = icon(&[
    "....y.....",
    ".y..y..y..",
    "...yyy....",
    "..yyyyy...",
    "yyyyyyyyy.",
    "..yyyyy...",
    "...yyy....",
    ".y..y..y..",
    "....y.....",
]);
pub static HOT_SUN: Sprite = icon(&[
    "....r.....",
    ".r..o..r..",
    "...ooo....",
    "..ooyoo...",
    "roooyyoor.",
    "..ooooo...",
    "...ooo....",
    ".r..o..r..",
    "....r.....",
]);
pub static CRATE: Sprite = icon(&[
    "bbbbbbbbbb",
    "bBttttttBb",
    "btBttttBtb",
    "bttBttBttb",
    "btttBBtttb",
    "bttBttBttb",
    "btBttttBtb",
    "bBttttttBb",
    "bbbbbbbbbb",
]);
pub static MAP: Sprite = icon(&[
    "tttttttttt",
    "tbttttGGtt",
    "ttbtttGGtt",
    "tttbbttttt",
    "tuutbttttt",
    "tuuutbbtrt",
    "ttuutttbrt",
    "ttttttttrt",
    "tttttttttt",
]);
pub static SLOW: Sprite = icon(&[
    "...g......",
    "...gg.....",
    "...ggg....",
    "...gggg...",
    "...ggg....",
    "...gg.....",
    "...g......",
]);
pub static FAST: Sprite = icon(&[
    ".y...y....",
    ".yy..yy...",
    ".yyy.yyy..",
    ".yyyyyyyy.",
    ".yyy.yyy..",
    ".yy..yy...",
    ".y...y....",
]);
pub static FASTEST: Sprite = icon(&[
    "r..r..r...",
    "rr.rr.rr..",
    "rrrrrrrrr.",
    "rrrrrrrrrr",
    "rrrrrrrrr.",
    "rr.rr.rr..",
    "r..r..r...",
]);
pub static DRUMSTICK: Sprite = icon(&[
    "...bbbb...",
    "..bbbbbb..",
    "..bobbbb..",
    "..bbbbob..",
    "...bbbb...",
    "....ww....",
    "....ww....",
    "...w..w...",
]);
pub static FULL_BOWL: Sprite = icon(&[
    "..obobbo..",
    ".obobbobo.",
    ".bbobobbo.",
    "aaaaaaaaaa",
    ".aaaaaaaa.",
    "..aaaaaa..",
    "...aaaa...",
]);
pub static HALF_BOWL: Sprite = icon(&[
    "..........",
    "..........",
    "...obob...",
    "aaaaaaaaaa",
    ".aaaaaaaa.",
    "..aaaaaa..",
    "...aaaa...",
]);
pub static BONE: Sprite = icon(&[
    "..........",
    "ww......ww",
    "www....www",
    ".wwwwwwww.",
    ".wwwwwwww.",
    "www....www",
    "ww......ww",
]);
pub static MOON: Sprite = icon(&[
    "....yyy...",
    "..yyy.....",
    ".yyy......",
    ".yy.......",
    ".yy.......",
    ".yyy......",
    "..yyy.....",
    "....yyy...",
]);
pub static SWAP: Sprite = icon(&[
    "......g...",
    "......gg..",
    "ggggggggg.",
    "......gg..",
    "..r...g...",
    ".rr.......",
    "rrrrrrrrr.",
    ".rr.......",
    "..r.......",
]);
pub static SPEECH: Sprite = icon(&[
    ".wwwwwwww.",
    "wwwwwwwwww",
    "wkwwkwwkww",
    "wwwwwwwwww",
    ".wwwwwwww.",
    "..ww......",
    ".w........",
]);
pub static TARGET: Sprite = icon(&[
    "...rrr....",
    ".rrwwwrr..",
    ".rwrrrwr..",
    "rwrwwwrwr.",
    "rwrwrwrwr.",
    "rwrwwwrwr.",
    ".rwrrrwr..",
    ".rrwwwrr..",
    "...rrr....",
]);
pub static FISH: Sprite = icon(&[
    "..........",
    "....uu....",
    "..uuuuu..u",
    ".uwuuuuuuu",
    ".uuuuuuuuu",
    "..uuuuu..u",
    "....uu....",
]);
pub static CHERRIES: Sprite = icon(&[
    ".....G....",
    "....G.G...",
    "...G...G..",
    "..G.....G.",
    ".rr....rr.",
    "rrrr..rrrr",
    "rrrr..rrrr",
    ".rr....rr.",
]);
pub static FLAG: Sprite = icon(&[
    ".awkwkwk..",
    ".akwkwkw..",
    ".awkwkwk..",
    ".akwkwkw..",
    ".a........",
    ".a........",
    ".a........",
    ".a........",
    ".a........",
]);
pub static WAVES: Sprite = icon(&[
    "..kk......",
    ".kaak.....",
    "..kk......",
    "..........",
    "u.uu.uu.u.",
    ".uu.uu.uu.",
    "uuuuuuuuuu",
    "UUUUUUUUUU",
]);
pub static RAFT: Sprite = icon(&[
    "....w.....",
    "....ww....",
    "....www...",
    "....w.....",
    "bbbbbbbbbb",
    ".bbbbbbbb.",
    "uuuuuuuuuu",
    ".u.uu.uu.u",
]);
pub static FERRY: Sprite = icon(&[
    "...aa.....",
    "...aa.....",
    ".rrrrrr...",
    "bbbbbbbbbb",
    "bbbbbbbbbb",
    ".bbbbbbbb.",
    "uuuuuuuuuu",
    ".u.uu.uu.u",
]);
pub static GUIDE: Sprite = icon(&[
    "...bbbb...",
    "..bbbbbb..",
    "...ssss...",
    "...ssss...",
    "..oooooo..",
    ".oooooooo.",
    "...oooo...",
    "...b..b...",
    "...b..b...",
]);
pub static HOURGLASS: Sprite = icon(&[
    ".bbbbbbbb.",
    "..yyyyyy..",
    "...yyyy...",
    "....yy....",
    "....yy....",
    "...y..y...",
    "..yyyyyy..",
    ".bbbbbbbb.",
]);
pub static ROAD: Sprite = icon(&[
    "....w.....",
    "...aww....",
    "..aaaaa...",
    ".aaaaaaa..",
    "aaaaaaaaa.",
    "....bb....",
    "...bb.....",
    "..bb......",
]);
pub static OX: Sprite = icon(&[
    "w........w",
    "ww......ww",
    ".wbbbbbbw.",
    "..bbbbbb..",
    "..bkbbkb..",
    "..bbbbbb..",
    "...bttb...",
    "...tttt...",
]);
pub static SHIRT: Sprite = icon(&[
    "..uu..uu..",
    ".uuuuuuuu.",
    "uuuuuuuuuu",
    "uu.uuuu.uu",
    "...uuuu...",
    "...uuuu...",
    "...uuuu...",
]);
pub static BULLETS: Sprite = icon(&[
    "..a....a..",
    ".aaa..aaa.",
    ".yyy..yyy.",
    ".yyy..yyy.",
    ".yyy..yyy.",
    ".yyy..yyy.",
    ".ooo..ooo.",
]);
pub static WHEEL: Sprite = icon(&[
    "..BBBBB...",
    ".B..b..B..",
    "B.b.b.b.B.",
    "B..bbb..B.",
    "BbbbBbbbB.",
    "B..bbb..B.",
    "B.b.b.b.B.",
    ".B..b..B..",
    "..BBBBB...",
]);
pub static AXLE: Sprite = icon(&[
    "..........",
    ".kk....kk.",
    "kaak..kaak",
    "kaabbbbaak",
    "kaak..kaak",
    ".kk....kk.",
]);
pub static TONGUE: Sprite = icon(&[
    ".........b",
    "........b.",
    ".......b..",
    "......b...",
    ".....b....",
    "bbbbbb....",
    "b....b....",
]);
pub static ANTENNA: Sprite = icon(&[
    "..uuuuu...",
    ".u.....u..",
    "u..uuu..u.",
    "..u...u...",
    "....u.....",
    "....a.....",
    "....a.....",
    "...aaa....",
]);
pub static ARROW: Sprite = icon(&[
    ".....g....",
    ".....gg...",
    "gggggggg..",
    "ggggggggg.",
    "gggggggg..",
    ".....gg...",
    ".....g....",
]);
pub static BACK: Sprite = icon(&[
    "...u......",
    "..uu......",
    ".uuuuuuuu.",
    "uuuuuuuuu.",
    ".uuuuuuuu.",
    "..uu......",
    "...u......",
]);
pub static CROSS: Sprite = icon(&[
    ".r.....r..",
    "rrr...rrr.",
    ".rrr.rrr..",
    "..rrrrr...",
    "...rrr....",
    "..rrrrr...",
    ".rrr.rrr..",
    "rrr...rrr.",
    ".r.....r..",
]);
pub static BALLOT: Sprite = icon(&[
    "...wwww...",
    "...wkkw...",
    "...wwww...",
    ".uuu..uuu.",
    ".uuuuuuuu.",
    ".uuuuuuuu.",
    ".uuuuuuuu.",
    ".uuuuuuuu.",
]);
pub static STAR: Sprite = icon(&[
    "....y.....",
    "....y.....",
    "...yyy....",
    "yyyyyyyyy.",
    ".yyyyyyy..",
    "..yyyyy...",
    "..yy.yy...",
    ".yy...yy..",
]);
pub static HELP: Sprite = icon(&[
    "....rr....",
    "....rr....",
    "....rr....",
    "....rr....",
    "....rr....",
    "..........",
    "....rr....",
    "....rr....",
]);

pub static RUNNER: Sprite = icon(&[
    "......bb..",
    "......ss..",
    "...uuuuu..",
    "..s..uu.s.",
    ".....uu...",
    ".....bbb..",
    "....b...b.",
    "..bb....b.",
    "..k.....kk",
]);
pub static SLED: Sprite = icon(&[
    ".....rrr..",
    ".....sss..",
    "....rrrr..",
    "...rrrrrs.",
    ".bbbbbbbbb",
    ".k......k.",
    "kkkkkkkkk.",
]);
pub static BALL: Sprite = icon(&[
    "...rrrr...",
    "..rrrrrr..",
    ".rwrrrrrr.",
    ".yyyyyyyy.",
    ".yyyyyyyy.",
    ".uuuuuuuu.",
    "..uuuuuu..",
    "...uuuu...",
]);

/// Words in a menu line, and its picture. Earlier rows win, so "Wagon wheels" is a wheel
/// and "Caulk the wagon and float" a raft. A word starting with ^ must begin the line
/// (or follow its "Name: "), so a saved trip's date doesn't turn into a month.
pub static PICKS: &[(&[&str], &Sprite)] = &[
    (&["host"], &ANTENNA),
    (&["join"], &ARROW),
    (&["^march"], &SNOW),
    (&["^april"], &RAIN),
    (&["^may"], &FLOWER),
    (&["^june"], &SUN),
    (&["^july"], &HOT_SUN),
    (&["younger"], &HEART),
    (&["seasoned"], &BOLT),
    (&["banker"], &MONEY),
    (&["carpenter"], &HAMMER),
    (&["farmer"], &WHEAT),
    (&["wheel"], &WHEEL),
    (&["axle"], &AXLE),
    (&["tongue"], &TONGUE),
    (&["caulk", "raft"], &RAFT),
    (&["ferry"], &FERRY),
    (&["guide"], &GUIDE),
    (&["ford", "river"], &WAVES),
    (&["barlow", "road"], &ROAD),
    (&["fish"], &FISH),
    (&["hunt"], &TARGET),
    (&["gather"], &CHERRIES),
    (&["food", "filling"], &FULL_BOWL),
    (&["meager"], &HALF_BOWL),
    (&["bare bones"], &BONE),
    (&["rations"], &DRUMSTICK),
    (&["steady", "pace"], &SLOW),
    (&["strenuous"], &FAST),
    (&["grueling"], &FASTEST),
    (&["ox", "oxen"], &OX),
    (&["clothing"], &SHIRT),
    (&["ammunition", "bullets"], &BULLETS),
    (&["wait"], &HOURGLASS),
    (&["rest"], &MOON),
    (&["trade"], &SWAP),
    (&["talk", "message"], &SPEECH),
    (&["buy"], &MONEY),
    (&["check supplies"], &CRATE),
    (&["map"], &MAP),
    (&["save", "saved"], &CHEST),
    (&["top ten"], &TROPHY),
    (&["fun and games", "play a game"], &BALL),
    (&["foot race"], &RUNNER),
    (&["sled"], &SLED),
    (&["wagon race"], &WAGON),
    (&["race"], &FLAG),
    (&["democracy", "vote"], &BALLOT),
    (&["back to"], &BACK),
    (&["leave the game"], &CROSS),
    (&["play together", "players"], &PEOPLE),
    (&["help"], &HELP),
    (&["luck", "woo", "cheats"], &STAR),
    (
        &["learn", "information", "advice", "differences", "size up"],
        &QUESTION,
    ),
    (&["travel", "continue", "wagon"], &WAGON),
];

/// The picture for a menu line's text (without its "1. "), if any word in it has one.
/// Words match at their start, so "ox" doesn't match "box".
pub fn for_label(label: &str) -> Option<&'static Sprite> {
    let low = label.to_ascii_lowercase();
    let value = low.split_once(": ").map_or(low.as_str(), |(_, v)| v);
    let has = |w: &str| {
        if let Some(w) = w.strip_prefix('^') {
            return low.starts_with(w) || value.starts_with(w);
        }
        low.match_indices(w).any(|(i, _)| {
            let before = low[..i].chars().next_back();
            let after = low[i + w.len()..].chars().next();
            !before.is_some_and(|c| c.is_ascii_alphabetic())
                && !after.is_some_and(|c| c.is_ascii_alphabetic() && w.len() < 3)
        })
    };
    PICKS
        .iter()
        .find(|(words, _)| words.iter().any(|w| has(w)))
        .map(|&(_, s)| s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is(label: &str, want: &Sprite) -> bool {
        for_label(label).is_some_and(|s| core::ptr::eq(s, want))
    }

    #[test]
    fn menu_lines_get_the_right_pictures() {
        let cases: &[(&str, &Sprite)] = &[
            ("Travel the trail", &WAGON),
            ("Learn about the trail", &QUESTION),
            ("See the Top Ten", &TROPHY),
            ("Load a saved trip", &CHEST),
            ("Play together", &PEOPLE),
            ("Fun and games", &BALL),
            ("Play a game", &BALL),
            ("Wagon race", &WAGON),
            ("Foot race", &RUNNER),
            ("Sled race", &SLED),
            ("Younger pioneers", &HEART),
            ("Banker from Boston", &MONEY),
            ("Find out the differences", &QUESTION),
            ("May", &FLOWER),
            ("Leave in: July", &HOT_SUN),
            ("Ask for advice", &QUESTION),
            ("Continue on trail", &WAGON),
            ("Check supplies", &CRATE),
            ("Change pace", &SLOW),
            ("Grueling", &FASTEST),
            ("Bare bones", &BONE),
            ("Stop to rest", &MOON),
            ("Go hunting", &TARGET),
            ("Go fishing", &FISH),
            ("Gather food", &CHERRIES),
            ("Wagon wheels", &WHEEL),
            ("Oxen", &OX),
            ("1 ox", &OX),
            ("Caulk the wagon and float", &RAFT),
            ("Attempt to ford the river", &WAVES),
            ("Hire a guide (3 sets of clothing)", &GUIDE),
            ("Raft down the Columbia River", &RAFT),
            ("Take the Barlow Road ($65)", &ROAD),
            ("Join Sam - Wagon race, 2/8", &ARROW),
            ("Game: Democracy", &BALLOT),
            ("Back to the game", &BACK),
            ("Leave the game", &CROSS),
            ("Need oxen!", &OX),
        ];
        for (label, want) in cases {
            assert!(is(label, want), "{label}");
        }
        // Names and saved trips get none, and a date's month doesn't count.
        assert!(for_label("Silas  (leader)").is_none());
        assert!(for_label("Fort Kearney, May 30, 1848").is_none());
        assert!(for_label("Boxer").is_none());
    }
}
