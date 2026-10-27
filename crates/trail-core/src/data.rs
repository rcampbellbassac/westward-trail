//! The trail "mod file": a forgiving `[section]` + `key = value` text format.
//!
//! Parsing never panics. Every mistake comes back as a [`DataError`] with the
//! line number, so a young modder can find and fix it.

use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};

/// The data file compiled into every build.
pub const DEFAULT_DATA: &str = include_str!("../data/trail.txt");

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Town,
    Fort,
    River,
    Landmark,
    Dalles,
    End,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Terrain {
    Plains,
    Hills,
    Mountains,
    Desert,
    Forest,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Skill {
    None,
    Repair,
    Oxen,
}

#[derive(Clone, Debug)]
pub struct Landmark {
    pub name: String,
    pub mile: u32,
    pub kind: Kind,
    pub terrain: Terrain,
    pub picture: String,
    /// River depth in tenths of a foot.
    pub depth10: u32,
    pub width: u32,
    pub ferry: u32,
    pub guide: bool,
    pub toll: u32,
    pub talk: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Animal {
    pub name: String,
    pub min_lbs: u32,
    pub max_lbs: u32,
    pub speed: u32,
    pub rarity: u32,
    /// Empty means "lives everywhere".
    pub terrain: Vec<Terrain>,
    pub sprite: String,
}

#[derive(Clone, Debug)]
pub struct Fish {
    pub name: String,
    pub min_lbs: u32,
    pub max_lbs: u32,
}

#[derive(Clone, Debug)]
pub struct Occupation {
    pub name: String,
    pub cash: u32,
    pub bonus: u32,
    pub skill: Skill,
    pub about: String,
}

#[derive(Clone, Debug)]
pub struct Illness {
    pub name: String,
    pub gentle: String,
}

/// Store prices in cents.
#[derive(Clone, Debug)]
pub struct Prices {
    pub store_name: String,
    pub oxen: u32,
    pub food: u32,
    pub clothing: u32,
    pub ammo: u32,
    pub part: u32,
}

#[derive(Clone, Debug)]
pub struct TrailData {
    pub title: String,
    pub year: u32,
    pub prices: Prices,
    pub occupations: Vec<Occupation>,
    pub landmarks: Vec<Landmark>,
    pub animals: Vec<Animal>,
    pub fish: Vec<Fish>,
    pub illnesses: Vec<Illness>,
    pub names: Vec<String>,
}

impl TrailData {
    pub fn trail_length(&self) -> u32 {
        self.landmarks.last().map_or(1, |l| l.mile.max(1))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataError {
    pub line: usize,
    pub message: String,
}

impl core::fmt::Display for DataError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.line == 0 {
            write!(f, "{}", self.message)
        } else {
            write!(f, "line {}: {}", self.line, self.message)
        }
    }
}

fn err<T>(line: usize, message: impl Into<String>) -> Result<T, DataError> {
    Err(DataError {
        line,
        message: message.into(),
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Section {
    Settings,
    Store,
    Occupation,
    Landmark,
    Animal,
    Fish,
    Illness,
    Names,
}

/// One section's raw key/value pairs, kept with line numbers for errors.
struct Block {
    section: Section,
    line: usize,
    pairs: Vec<(usize, String, String)>,
}

impl Block {
    fn get(&self, key: &str) -> Option<(usize, &str)> {
        self.pairs
            .iter()
            .rev()
            .find(|(_, k, _)| k == key)
            .map(|(l, _, v)| (*l, v.as_str()))
    }

    fn all(&self, key: &str) -> impl Iterator<Item = &str> {
        self.pairs
            .iter()
            .filter(move |(_, k, _)| k == key)
            .map(|(_, _, v)| v.as_str())
    }

    fn text(&self, key: &str) -> Result<String, DataError> {
        match self.get(key) {
            Some((_, v)) if !v.is_empty() => Ok(v.to_string()),
            _ => err(
                self.line,
                format!("this section needs a \"{key} = ...\" line"),
            ),
        }
    }

    fn text_or(&self, key: &str, default: &str) -> String {
        self.get(key).map_or(default, |(_, v)| v).to_string()
    }

    fn number(&self, key: &str) -> Result<u32, DataError> {
        let (line, v) = self.get(key).ok_or_else(|| DataError {
            line: self.line,
            message: format!("missing \"{key} = <number>\""),
        })?;
        parse_number(line, v)
    }

    fn number_or(&self, key: &str, default: u32) -> Result<u32, DataError> {
        match self.get(key) {
            Some((line, v)) => parse_number(line, v),
            None => Ok(default),
        }
    }

    /// Decimal value scaled by 10^places, so "4.5" with 1 place is 45.
    fn decimal_or(&self, key: &str, places: u32, default: u32) -> Result<u32, DataError> {
        match self.get(key) {
            Some((line, v)) => parse_decimal(line, v, places),
            None => Ok(default),
        }
    }
}

fn parse_number(line: usize, v: &str) -> Result<u32, DataError> {
    v.trim()
        .trim_start_matches('$')
        .parse::<u32>()
        .or_else(|_| err(line, format!("\"{v}\" should be a whole number like 42")))
}

fn parse_decimal(line: usize, v: &str, places: u32) -> Result<u32, DataError> {
    let v = v.trim().trim_start_matches('$');
    let (whole, frac) = v.split_once('.').unwrap_or((v, ""));
    let bad = || DataError {
        line,
        message: format!("\"{v}\" should be a number like 4.5"),
    };
    let whole: u32 = if whole.is_empty() {
        0
    } else {
        whole.parse().map_err(|_| bad())?
    };
    let mut frac_val = 0u32;
    for (i, c) in frac.chars().enumerate() {
        let d = c.to_digit(10).ok_or_else(bad)?;
        if (i as u32) < places {
            frac_val += d * 10u32.pow(places - 1 - i as u32);
        }
    }
    whole
        .checked_mul(10u32.pow(places))
        .and_then(|w| w.checked_add(frac_val))
        .ok_or_else(|| DataError {
            line,
            message: format!("\"{v}\" is too big"),
        })
}

fn parse_terrain(line: usize, v: &str) -> Result<Terrain, DataError> {
    Ok(match v.trim().to_ascii_lowercase().as_str() {
        "plains" => Terrain::Plains,
        "hills" => Terrain::Hills,
        "mountains" => Terrain::Mountains,
        "desert" => Terrain::Desert,
        "forest" => Terrain::Forest,
        other => {
            return err(
                line,
                format!(
                    "unknown terrain \"{other}\" (try plains, hills, mountains, desert, forest)"
                ),
            );
        }
    })
}

pub fn parse(src: &str) -> Result<TrailData, DataError> {
    let mut blocks: Vec<Block> = Vec::new();
    for (i, raw) in src.lines().enumerate() {
        let line = i + 1;
        let text = raw.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        if let Some(name) = text.strip_prefix('[') {
            let Some(name) = name.strip_suffix(']') else {
                return err(line, "a section name needs a closing ]");
            };
            let section = match name.trim().to_ascii_lowercase().as_str() {
                "settings" => Section::Settings,
                "store" => Section::Store,
                "occupation" => Section::Occupation,
                "landmark" => Section::Landmark,
                "animal" => Section::Animal,
                "fish" => Section::Fish,
                "illness" => Section::Illness,
                "names" => Section::Names,
                other => return err(line, format!("unknown section [{other}]")),
            };
            blocks.push(Block {
                section,
                line,
                pairs: Vec::new(),
            });
            continue;
        }
        let Some((key, value)) = text.split_once('=') else {
            return err(line, "expected  key = value  (or a [section])");
        };
        let Some(block) = blocks.last_mut() else {
            return err(line, "put a [section] line above this");
        };
        block.pairs.push((
            line,
            key.trim().to_ascii_lowercase(),
            value.trim().to_string(),
        ));
    }

    let mut data = TrailData {
        title: "Westward Trail".to_string(),
        year: 1848,
        prices: Prices {
            store_name: "General Store".to_string(),
            oxen: 4000,
            food: 20,
            clothing: 1000,
            ammo: 200,
            part: 1000,
        },
        occupations: Vec::new(),
        landmarks: Vec::new(),
        animals: Vec::new(),
        fish: Vec::new(),
        illnesses: Vec::new(),
        names: Vec::new(),
    };

    for b in &blocks {
        match b.section {
            Section::Settings => {
                data.title = b.text_or("title", &data.title);
                data.year = b.number_or("year", data.year)?;
            }
            Section::Store => {
                let p = &mut data.prices;
                p.store_name = b.text_or("name", &p.store_name);
                p.oxen = b.decimal_or("oxen", 2, p.oxen)?;
                p.food = b.decimal_or("food", 2, p.food)?;
                p.clothing = b.decimal_or("clothing", 2, p.clothing)?;
                p.ammo = b.decimal_or("ammo", 2, p.ammo)?;
                p.part = b.decimal_or("part", 2, p.part)?;
            }
            Section::Occupation => {
                let skill = match b.text_or("skill", "none").to_ascii_lowercase().as_str() {
                    "repair" => Skill::Repair,
                    "oxen" => Skill::Oxen,
                    "none" | "" => Skill::None,
                    other => {
                        return err(
                            b.line,
                            format!("unknown skill \"{other}\" (try none, repair, oxen)"),
                        );
                    }
                };
                data.occupations.push(Occupation {
                    name: b.text("name")?,
                    cash: b.number("cash")?,
                    bonus: b.number_or("bonus", 1)?.max(1),
                    skill,
                    about: b.text_or("about", ""),
                });
            }
            Section::Landmark => {
                let (kline, kind) = b.get("kind").unwrap_or((b.line, "landmark"));
                let kind = match kind.to_ascii_lowercase().as_str() {
                    "town" => Kind::Town,
                    "fort" => Kind::Fort,
                    "river" => Kind::River,
                    "landmark" => Kind::Landmark,
                    "dalles" => Kind::Dalles,
                    "end" => Kind::End,
                    other => {
                        return err(
                            kline,
                            format!(
                                "unknown kind \"{other}\" (try town, fort, river, landmark, dalles, end)"
                            ),
                        );
                    }
                };
                let terrain = match b.get("terrain") {
                    Some((l, v)) => parse_terrain(l, v)?,
                    None => Terrain::Plains,
                };
                let guide = matches!(
                    b.text_or("guide", "no").to_ascii_lowercase().as_str(),
                    "yes" | "true" | "1"
                );
                data.landmarks.push(Landmark {
                    name: b.text("name")?,
                    mile: b.number("mile")?,
                    kind,
                    terrain,
                    picture: b.text_or("picture", "rock"),
                    depth10: b.decimal_or("depth", 1, 30)?,
                    width: b.number_or("width", 300)?,
                    ferry: b.number_or("ferry", 0)?,
                    guide,
                    toll: b.number_or("toll", 30)?,
                    talk: b.all("talk").map(ToString::to_string).collect(),
                });
            }
            Section::Animal => {
                let mut terrain = Vec::new();
                if let Some((l, v)) = b.get("terrain") {
                    for part in v.split(',').filter(|p| !p.trim().is_empty()) {
                        terrain.push(parse_terrain(l, part)?);
                    }
                }
                let min_lbs = b.number("min_lbs")?;
                let max_lbs = b.number("max_lbs")?.max(min_lbs);
                data.animals.push(Animal {
                    name: b.text("name")?,
                    min_lbs,
                    max_lbs,
                    speed: b.number_or("speed", 40)?.clamp(1, 400),
                    rarity: b.number_or("rarity", 10)?,
                    terrain,
                    sprite: b.text_or("sprite", "rabbit"),
                });
            }
            Section::Fish => {
                let min_lbs = b.number_or("min_lbs", 1)?;
                data.fish.push(Fish {
                    name: b.text("name")?,
                    min_lbs,
                    max_lbs: b.number_or("max_lbs", min_lbs)?.max(min_lbs),
                });
            }
            Section::Illness => {
                let name = b.text("name")?;
                let gentle = b.text_or("gentle", &name);
                data.illnesses.push(Illness { name, gentle });
            }
            Section::Names => {
                for list in b.all("list") {
                    data.names.extend(
                        list.split(',')
                            .map(str::trim)
                            .filter(|n| !n.is_empty())
                            .map(ToString::to_string),
                    );
                }
            }
        }
    }

    validate(&data, &blocks)?;
    Ok(data)
}

fn validate(data: &TrailData, blocks: &[Block]) -> Result<(), DataError> {
    let landmark_lines: Vec<usize> = blocks
        .iter()
        .filter(|b| b.section == Section::Landmark)
        .map(|b| b.line)
        .collect();
    if data.landmarks.len() < 2 {
        return err(0, "the trail needs at least two [landmark] sections");
    }
    if data.landmarks[0].mile != 0 {
        return err(landmark_lines[0], "the first landmark must be at mile = 0");
    }
    for i in 1..data.landmarks.len() {
        if data.landmarks[i].mile <= data.landmarks[i - 1].mile {
            return err(
                landmark_lines[i],
                format!(
                    "landmarks must go in order; this one should be farther than mile {}",
                    data.landmarks[i - 1].mile
                ),
            );
        }
    }
    let last = data.landmarks.len() - 1;
    if data.landmarks[last].kind != Kind::End {
        return err(landmark_lines[last], "the last landmark needs  kind = end");
    }
    if let Some(i) = data.landmarks[..last]
        .iter()
        .position(|l| l.kind == Kind::End)
    {
        return err(
            landmark_lines[i],
            "only the last landmark can be kind = end",
        );
    }
    if data.occupations.is_empty() {
        return err(0, "add at least one [occupation]");
    }
    if data.animals.is_empty() {
        return err(0, "add at least one [animal] so there's something to hunt");
    }
    if data.illnesses.is_empty() {
        return err(0, "add at least one [illness]");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_data_parses() {
        let d = parse(DEFAULT_DATA).expect("default data");
        assert_eq!(d.occupations.len(), 3);
        assert_eq!(d.prices.food, 20);
        assert_eq!(d.landmarks[1].depth10, 45);
        assert_eq!(d.landmarks.last().unwrap().kind, Kind::End);
        assert!(d.names.len() >= 5);
    }

    #[test]
    fn errors_point_at_lines() {
        let e = parse("[landmark]\nname = A\nmile = zero\n").unwrap_err();
        assert_eq!(e.line, 3);
        let e = parse("[landmark]\nname = A\nmile = 0\nkind = moon\n").unwrap_err();
        assert_eq!(e.line, 4);
        let e = parse("[dragons]\n").unwrap_err();
        assert_eq!(e.line, 1);
    }

    #[test]
    fn decimals() {
        assert_eq!(parse_decimal(1, "0.2", 2).unwrap(), 20);
        assert_eq!(parse_decimal(1, "$40", 2).unwrap(), 4000);
        assert_eq!(parse_decimal(1, "4.55", 1).unwrap(), 45);
    }
}
