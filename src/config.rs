use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt;

use serde::Deserialize;
use serde::de::{self, Deserializer, SeqAccess, Visitor};
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::keys::{Action, Combo, Slider, Toggle};
use crate::settings::Config;
use crate::symbols::Template;
use crate::zones::Corner;

pub const LIMIT: u64 = 1024 * 1024;
pub const SYMBOLS: &str = "symbols.toml";
const MAX_GRID: usize = 8;
const MAX_KNOCK: usize = 8;
const MAX_NAME: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

fn problem(text: &str, error: &toml::de::Error) -> Problem {
    let start = error.span().map_or(0, |span| span.start).min(text.len());
    let before = text.get(..start).unwrap_or_default();
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().map_or(0, |tail| tail.chars().count()) + 1;
    Problem { line, column, message: error.message().split('\n').next().unwrap_or_default().trim().to_string() }
}

pub fn parse(text: &str) -> Result<Config, Problem> {
    toml::from_str(text).map_err(|e| problem(text, &e))
}

#[derive(Debug)]
pub enum LoadError {
    Unreadable(PathBuf, std::io::Error),
    TooBig(PathBuf),
    Invalid(PathBuf, Problem),
}

fn read(path: &Path) -> Result<String, LoadError> {
    let unreadable = |error| LoadError::Unreadable(path.to_path_buf(), error);
    let file = std::fs::File::open(path).map_err(unreadable)?;
    let mut text = String::new();
    file.take(LIMIT + 1).read_to_string(&mut text).map_err(unreadable)?;
    if text.len() as u64 > LIMIT {
        return Err(LoadError::TooBig(path.to_path_buf()));
    }
    Ok(text)
}

pub fn symbols_beside(path: &Path) -> PathBuf {
    path.with_file_name(SYMBOLS)
}

pub fn load_symbols(path: &Path) -> Result<Vec<Template>, LoadError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    templates(&read(path)?).map_err(|problem| LoadError::Invalid(path.to_path_buf(), problem))
}

pub fn load(path: &Path) -> Result<(Config, Vec<Template>), LoadError> {
    let config = parse(&read(path)?).map_err(|problem| LoadError::Invalid(path.to_path_buf(), problem))?;
    Ok((config, load_symbols(&symbols_beside(path))?))
}

pub fn millis<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    u64::deserialize(deserializer)?.checked_mul(1000).ok_or_else(|| de::Error::custom("the time is too long"))
}

fn action(text: &str) -> Result<Action, String> {
    Action::parse(text).ok_or_else(|| format!("unknown action `{text}`"))
}

fn unlockable(text: &str) -> Result<Action, String> {
    let found = action(text)?;
    if found == Action::Toggle(Toggle::Touchpad) {
        return Err(format!("`{text}` works only on a corner or a knock code: a switched-off touchpad listens to nothing else"));
    }
    Ok(found)
}

fn optional<'de, D: Deserializer<'de>>(deserializer: D, read: fn(&str) -> Result<Action, String>) -> Result<Option<Action>, D::Error> {
    let text = String::deserialize(deserializer)?;
    if text.trim() == "none" {
        return Ok(None);
    }
    read(&text).map(Some).map_err(de::Error::custom)
}

pub fn binding<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Action>, D::Error> {
    optional(deserializer, unlockable)
}

pub fn corner_binding<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Option<Action>, D::Error> {
    optional(deserializer, action)
}

pub fn codes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<(Vec<u8>, Action)>, D::Error> {
    let map = BTreeMap::<String, String>::deserialize(deserializer)?;
    map.into_iter()
        .map(|(code, name)| {
            let quadrants: Option<Vec<u8>> =
                code.chars().map(|digit| digit.to_digit(10).and_then(|value| u8::try_from(value).ok()).filter(|value| (1..=4).contains(value))).collect();
            let quadrants = quadrants
                .filter(|quadrants| (2..=MAX_KNOCK).contains(&quadrants.len()))
                .ok_or_else(|| de::Error::custom(format!("a knock code is 2 to {MAX_KNOCK} digits from 1 to 4, not `{code}`")))?;
            Ok((quadrants, action(&name).map_err(de::Error::custom)?))
        })
        .collect()
}

pub fn named<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<(String, Action)>, D::Error> {
    let map = BTreeMap::<String, String>::deserialize(deserializer)?;
    map.into_iter().map(|(name, text)| Ok((name, unlockable(&text).map_err(de::Error::custom)?))).collect()
}

pub fn rows<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Vec<Option<Combo>>>, D::Error> {
    let rows = Vec::<Vec<String>>::deserialize(deserializer)?;
    if rows.is_empty() || rows.len() > MAX_GRID || rows.iter().any(|row| row.is_empty() || row.len() > MAX_GRID) {
        return Err(de::Error::custom(format!("the numpad has 1 to {MAX_GRID} rows of 1 to {MAX_GRID} keys")));
    }
    rows.into_iter()
        .map(|row| {
            row.into_iter()
                .map(|name| match name.trim() {
                    "none" => Ok(None),
                    key => Combo::parse(key).map(Some).ok_or_else(|| de::Error::custom(format!("unknown key `{key}`"))),
                })
                .collect()
        })
        .collect()
}

struct SliderVisitor;

impl<'de> Visitor<'de> for SliderVisitor {
    type Value = Slider;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a slider name or a pair of actions")
    }

    fn visit_str<E: de::Error>(self, name: &str) -> Result<Slider, E> {
        Slider::preset(name).ok_or_else(|| E::custom(format!("unknown slider `{name}`")))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Slider, A::Error> {
        let forward: String = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let back: String = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
        if seq.next_element::<String>()?.is_some() {
            return Err(de::Error::invalid_length(3, &self));
        }
        Slider::pair(&forward, &back).ok_or_else(|| de::Error::custom(format!("unknown actions `{forward}`, `{back}`")))
    }
}

impl<'de> Deserialize<'de> for Slider {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(SliderVisitor)
    }
}

impl<'de> Deserialize<'de> for Corner {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Corner::parse(&text).ok_or_else(|| de::Error::custom(format!("unknown corner `{text}`")))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Learned {
    #[serde(default)]
    symbol: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    name: String,
    points: Vec<[f32; 2]>,
}

pub fn valid_name(name: &str) -> bool {
    !name.is_empty() && name.len() <= MAX_NAME && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

pub fn templates(text: &str) -> Result<Vec<Template>, Problem> {
    let learned: Learned = toml::from_str(text).map_err(|e| problem(text, &e))?;
    learned
        .symbol
        .into_iter()
        .map(|entry| {
            if !valid_name(&entry.name) {
                return Err(Problem { line: 0, column: 0, message: format!("bad symbol name `{}`", entry.name) });
            }
            Ok(Template { name: entry.name, points: entry.points.into_iter().map(|[x, y]| (x, y)).collect() })
        })
        .collect()
}

pub fn entry(template: &Template) -> Option<String> {
    if !valid_name(&template.name) {
        return None;
    }
    let points: Vec<String> = template.points.iter().map(|(x, y)| format!("[{x:.4}, {y:.4}]")).collect();
    Some(format!("[[symbol]]\nname = \"{}\"\npoints = [{}]\n", template.name, points.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keys::{Key, Toggle};
    use crate::{Button, DEFAULT};

    #[test]
    fn an_empty_file_changes_nothing() {
        let config = parse("").unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.pointer, DEFAULT);
    }

    #[test]
    fn sections_times_actions_and_sliders_are_read() {
        let config = parse(
            r#"
            [pointer]
            speed = 5.0
            natural-scroll = true
            tap-time = 250

            [inertia]
            enabled = true
            friction = 2.5

            [taps]
            three = "draw"
            four = "none"

            [corners]
            enabled = true
            top-left.double-tap = "toggle-touchpad"
            top-right.hold = "numpad"

            [knock]
            enabled = true
            codes = { 1214 = "lock-screen" }

            [rails]
            enabled = true
            sides = "volume"
            top-bottom = ["brightness-up", "brightness-down"]
            right = "scroll"

            [joystick]
            enabled = true
            corner = "bottom-left"

            [symbols]
            enabled = true
            actions = { circle = "calculator", check = "ctrl+s" }

            [numpad]
            enabled = true
            rows = [["7", "8", "9"], ["none", "0", "enter"]]

            [typing]
            enabled = true
            quiet = 300
            "#,
        )
        .unwrap();
        assert_eq!(config.pointer.speed, 5.0);
        assert!(config.pointer.natural_scroll);
        assert_eq!(config.pointer.tap_time, 250_000);
        assert_eq!(config.pointer.tap_travel, DEFAULT.tap_travel);
        assert!(config.inertia.enabled && config.inertia.friction == 2.5);
        assert_eq!(config.taps.three, Some(Action::Draw));
        assert_eq!(config.taps.four, None);
        assert_eq!(config.taps.one, Some(Action::Click(Button::Left)));
        assert_eq!(config.corners.top_left.double_tap, Some(Action::Toggle(Toggle::Touchpad)));
        assert_eq!(config.corners.top_right.hold, Some(Action::Toggle(Toggle::Numpad)));
        assert_eq!(config.knock.codes, [(alloc::vec![1, 2, 1, 4], Action::Keys(Combo::of(&[Key::LockScreen]).unwrap()))]);
        assert_eq!(config.rails.sides, Slider::preset("volume").unwrap());
        assert_eq!(config.rails.top_bottom, Slider::preset("brightness").unwrap());
        assert_eq!(config.rails.right, Slider::Scroll);
        assert_eq!(config.joystick.corner, Corner::BottomLeft);
        assert_eq!(config.symbols.action("circle"), Action::parse("calculator"));
        assert_eq!(config.numpad.rows[1], [None, Combo::parse("0"), Combo::parse("enter")]);
        assert!(config.typing.enabled && config.typing.quiet == 300_000);
    }

    #[test]
    fn mistakes_are_reported_instead_of_ignored() {
        for broken in [
            "[pointer]\nsped = 3.0",
            "[taps]\none = \"jump\"",
            "[rails]\nsides = \"loudness\"",
            "[rails]\nsides = [\"volume-up\"]",
            "[knock]\ncodes = { 15 = \"mute\" }",
            "[numpad]\nrows = []",
            "[joystick]\ncorner = \"middle\"",
            "[wobble]\nenabled = true",
        ] {
            assert!(parse(broken).is_err(), "{broken}");
        }
        let problem = parse("[pointer]\nspeed = 2.0\n\n[taps]\none = \"jump\"\n").unwrap_err();
        assert_eq!((problem.line, problem.column), (5, 7));
        assert_eq!(problem.message, "unknown action `jump`");
        for elsewhere in ["[shake]\naction = \"toggle-touchpad\"", "[cover]\naction = \"toggle-touchpad\"", "[taps]\nthree = \"toggle-touchpad\""] {
            let problem = parse(elsewhere).unwrap_err();
            assert!(problem.message.contains("only on a corner or a knock code"), "{elsewhere}: {}", problem.message);
        }
        assert!(parse("[knock]\nenabled = true\ncodes = { 1234 = \"toggle-touchpad\" }").is_ok());
    }

    #[test]
    fn the_example_configuration_is_valid() {
        let config = parse(include_str!("../docs/example.toml")).unwrap();
        assert!(config.inertia.enabled);
    }

    #[test]
    fn the_settings_and_the_learned_symbols_beside_them_are_read_together() {
        let dir = std::env::temp_dir().join(alloc::format!("gestus-config-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("gestures.toml");
        std::fs::write(&path, "[inertia]\nenabled = true\n").unwrap();
        let (config, symbols) = load(&path).unwrap();
        assert!(config.inertia.enabled && symbols.is_empty());
        let saw = Template { name: String::from("saw"), points: alloc::vec![(0.0, 0.0), (1.0, 1.0)] };
        std::fs::write(symbols_beside(&path), entry(&saw).unwrap()).unwrap();
        assert_eq!(load(&path).unwrap().1, [saw]);
        std::fs::write(&path, "[inertia]\nenabled = 1\n").unwrap();
        assert!(matches!(load(&path), Err(LoadError::Invalid(_, Problem { line: 2, .. }))));
        std::fs::write(&path, alloc::vec![b' '; LIMIT as usize + 2]).unwrap();
        assert!(matches!(load(&path), Err(LoadError::TooBig(_))));
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(matches!(load(&path), Err(LoadError::Unreadable(..))));
    }

    #[test]
    fn learned_symbols_go_back_and_forth_through_text() {
        let template = Template { name: String::from("saw-1"), points: alloc::vec![(-0.5, 0.25), (0.5, -0.25)] };
        let text = entry(&template).unwrap();
        assert_eq!(templates(&text).unwrap(), [template]);
        assert_eq!(entry(&Template { name: String::from("bad name"), points: Vec::new() }), None);
        assert!(templates("[[symbol]]\nname = \"x y\"\npoints = []").is_err());
    }
}
