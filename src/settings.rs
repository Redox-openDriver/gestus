use alloc::string::String;
use alloc::vec::Vec;

use crate::Button;
use crate::keys::{Action, Combo, Slider};
use crate::zones::Corner;

#[cfg(feature = "config")]
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Settings {
    pub speed: f32,
    pub acceleration: f32,
    pub slow: f32,
    pub fast: f32,
    pub tap: bool,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub tap_time: u64,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub multi_tap_time: u64,
    pub tap_travel: f32,
    pub scroll_speed: f32,
    pub scroll_start: f32,
    pub natural_scroll: bool,
    pub swipe_start: f32,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub gap: u64,
    pub thumb_zone: f32,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub rest_time: u64,
    pub rest_travel: f32,
    pub edge: f32,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub edge_rest_time: u64,
    pub side_edge: f32,
    pub bottom_edge: f32,
    pub edge_release: f32,
    pub follow: f32,
}

pub const DEFAULT: Settings = Settings {
    speed: 4.0,
    acceleration: 3.0,
    slow: 30.0,
    fast: 300.0,
    tap: true,
    tap_time: 220_000,
    multi_tap_time: 360_000,
    tap_travel: 2.5,
    scroll_speed: 6.0,
    scroll_start: 2.0,
    natural_scroll: false,
    swipe_start: 5.0,
    gap: 100_000,
    thumb_zone: 0.75,
    rest_time: 300_000,
    rest_travel: 3.0,
    edge: 4.0,
    edge_rest_time: 500_000,
    side_edge: 3.0,
    bottom_edge: 3.0,
    edge_release: 8.0,
    follow: 0.45,
};

impl Default for Settings {
    fn default() -> Self {
        DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Inertia {
    pub enabled: bool,
    pub friction: f32,
    pub start: f32,
    pub stop: f32,
}

impl Default for Inertia {
    fn default() -> Self {
        Inertia { enabled: false, friction: 3.0, start: 60.0, stop: 3.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Throw {
    pub enabled: bool,
    pub friction: f32,
    pub start: f32,
    pub stop: f32,
}

impl Default for Throw {
    fn default() -> Self {
        Throw { enabled: false, friction: 6.0, start: 250.0, stop: 15.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Taps {
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub one: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub two: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub three: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub four: Option<Action>,
    pub drag: bool,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub drag_time: u64,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub window: u64,
}

impl Default for Taps {
    fn default() -> Self {
        Taps {
            one: Some(Action::Click(Button::Left)),
            two: Some(Action::Click(Button::Right)),
            three: Some(Action::Click(Button::Middle)),
            four: Some(Action::Click(Button::Middle)),
            drag: false,
            drag_time: 180_000,
            window: 300_000,
        }
    }
}

impl Taps {
    pub fn binding(&self, fingers: u8) -> Option<Action> {
        match fingers {
            0 | 1 => self.one,
            2 => self.two,
            3 => self.three,
            _ => self.four,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct CornerActions {
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::corner_binding"))]
    pub tap: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::corner_binding"))]
    pub double_tap: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::corner_binding"))]
    pub hold: Option<Action>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Corners {
    pub enabled: bool,
    pub size: f32,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub hold_time: u64,
    pub top_left: CornerActions,
    pub top_right: CornerActions,
    pub bottom_left: CornerActions,
    pub bottom_right: CornerActions,
}

impl Default for Corners {
    fn default() -> Self {
        let none = CornerActions::default();
        Corners { enabled: false, size: 10.0, hold_time: 1_000_000, top_left: none, top_right: none, bottom_left: none, bottom_right: none }
    }
}

impl Corners {
    pub fn get(&self, corner: Corner) -> &CornerActions {
        match corner {
            Corner::TopLeft => &self.top_left,
            Corner::TopRight => &self.top_right,
            Corner::BottomLeft => &self.bottom_left,
            Corner::BottomRight => &self.bottom_right,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Knock {
    pub enabled: bool,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub window: u64,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::codes"))]
    pub codes: Vec<(Vec<u8>, Action)>,
}

impl Default for Knock {
    fn default() -> Self {
        Knock { enabled: false, window: 450_000, codes: Vec::new() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Drag {
    pub enabled: bool,
    pub fingers: u8,
    pub start: f32,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub grace: u64,
}

impl Default for Drag {
    fn default() -> Self {
        Drag { enabled: false, fingers: 3, start: 3.0, grace: 500_000 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct EdgeMotion {
    pub enabled: bool,
    pub width: f32,
    pub speed: f32,
}

impl Default for EdgeMotion {
    fn default() -> Self {
        EdgeMotion { enabled: false, width: 5.0, speed: 40.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Directions {
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub left: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub right: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub up: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub down: Option<Action>,
}

impl Directions {
    pub fn toward(&self, dx: f32, dy: f32) -> Option<Action> {
        if dx.abs() >= dy.abs() {
            if dx < 0.0 { self.left } else { self.right }
        } else if dy < 0.0 {
            self.up
        } else {
            self.down
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Swipes {
    pub enabled: bool,
    pub three: Directions,
    pub four: Directions,
    pub switcher: u8,
    pub step: f32,
}

impl Default for Swipes {
    fn default() -> Self {
        Swipes { enabled: false, three: Directions::default(), four: Directions::default(), switcher: 0, step: 12.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Rails {
    pub enabled: bool,
    pub width: f32,
    pub step: f32,
    pub sides: Slider,
    pub top_bottom: Slider,
    pub left: Slider,
    pub right: Slider,
    pub top: Slider,
    pub bottom: Slider,
}

impl Default for Rails {
    fn default() -> Self {
        Rails {
            enabled: false,
            width: 10.0,
            step: 4.0,
            sides: Slider::Off,
            top_bottom: Slider::Off,
            left: Slider::Off,
            right: Slider::Off,
            top: Slider::Off,
            bottom: Slider::Off,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Circular {
    pub enabled: bool,
    pub width: f32,
    pub action: Slider,
    pub step: f32,
    pub turn: f32,
}

impl Default for Circular {
    fn default() -> Self {
        Circular { enabled: false, width: 8.0, action: Slider::Scroll, step: 30.0, turn: 90.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Rotate {
    pub enabled: bool,
    pub action: Slider,
    pub step: f32,
}

impl Default for Rotate {
    fn default() -> Self {
        Rotate { enabled: false, action: Slider::preset("volume").unwrap_or_default(), step: 30.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Pinch {
    pub enabled: bool,
    pub action: Slider,
    pub step: f32,
}

impl Default for Pinch {
    fn default() -> Self {
        Pinch { enabled: false, action: Slider::preset("zoom").unwrap_or_default(), step: 6.0 }
    }
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Symbols {
    pub enabled: bool,
    pub threshold: f32,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub timeout: u64,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::named"))]
    pub actions: Vec<(String, Action)>,
}

impl Default for Symbols {
    fn default() -> Self {
        Symbols { enabled: false, threshold: 0.8, timeout: 3_000_000, actions: Vec::new() }
    }
}

impl Symbols {
    pub fn action(&self, name: &str) -> Option<Action> {
        self.actions.iter().find(|(symbol, _)| symbol == name).map(|(_, action)| *action)
    }
}

pub const NUMPAD: [[&str; 5]; 4] = [
    ["7", "8", "9", "kp-slash", "backspace"],
    ["4", "5", "6", "kp-asterisk", "backspace"],
    ["1", "2", "3", "kp-minus", "enter"],
    ["0", "0", "dot", "kp-plus", "enter"],
];

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Numpad {
    pub enabled: bool,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::rows"))]
    pub rows: Vec<Vec<Option<Combo>>>,
}

impl Default for Numpad {
    fn default() -> Self {
        Numpad { enabled: false, rows: NUMPAD.iter().map(|row| row.iter().map(|name| Combo::parse(name)).collect()).collect() }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Sniper {
    pub enabled: bool,
    pub factor: f32,
}

impl Default for Sniper {
    fn default() -> Self {
        Sniper { enabled: false, factor: 0.35 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Joystick {
    pub enabled: bool,
    pub corner: Corner,
    pub size: f32,
    pub speed: f32,
    pub dead: f32,
}

impl Default for Joystick {
    fn default() -> Self {
        Joystick { enabled: false, corner: Corner::BottomRight, size: 20.0, speed: 25.0, dead: 1.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Dwell {
    pub enabled: bool,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub time: u64,
    pub still: f32,
    pub travel: f32,
}

impl Default for Dwell {
    fn default() -> Self {
        Dwell { enabled: false, time: 800_000, still: 0.6, travel: 8.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Smoothing {
    pub enabled: bool,
    pub cutoff: f32,
    pub beta: f32,
}

impl Default for Smoothing {
    fn default() -> Self {
        Smoothing { enabled: false, cutoff: 2.0, beta: 0.05 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Cover {
    pub enabled: bool,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub action: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub time: u64,
    pub contacts: u8,
}

impl Default for Cover {
    fn default() -> Self {
        Cover { enabled: false, action: Action::parse("play-pause"), time: 500_000, contacts: 1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Shake {
    pub enabled: bool,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub action: Option<Action>,
    pub turns: u8,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub window: u64,
    pub amplitude: f32,
}

impl Default for Shake {
    fn default() -> Self {
        Shake { enabled: false, action: Action::parse("locate"), turns: 5, window: 900_000, amplitude: 6.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Tablet {
    pub enabled: bool,
    pub draw_on_touch: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct EdgeSwipes {
    pub enabled: bool,
    pub width: f32,
    pub distance: f32,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub time: u64,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub left: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub right: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub top: Option<Action>,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::binding"))]
    pub bottom: Option<Action>,
}

impl Default for EdgeSwipes {
    fn default() -> Self {
        EdgeSwipes { enabled: false, width: 3.0, distance: 10.0, time: 400_000, left: None, right: None, top: None, bottom: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Typing {
    pub enabled: bool,
    #[cfg_attr(feature = "config", serde(deserialize_with = "crate::config::millis"))]
    pub quiet: u64,
}

impl Default for Typing {
    fn default() -> Self {
        Typing { enabled: false, quiet: 500_000 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Areas {
    pub enabled: bool,
    pub width: f32,
    pub height: f32,
}

impl Default for Areas {
    fn default() -> Self {
        Areas { enabled: false, width: 0.4, height: 15.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "config", derive(Deserialize), serde(default, deny_unknown_fields, rename_all = "kebab-case"))]
pub struct Config {
    pub pointer: Settings,
    pub inertia: Inertia,
    pub throw: Throw,
    pub taps: Taps,
    pub corners: Corners,
    pub knock: Knock,
    pub drag: Drag,
    pub edge_motion: EdgeMotion,
    pub swipes: Swipes,
    pub rails: Rails,
    pub circular: Circular,
    pub rotate: Rotate,
    pub pinch: Pinch,
    pub symbols: Symbols,
    pub numpad: Numpad,
    pub sniper: Sniper,
    pub joystick: Joystick,
    pub dwell: Dwell,
    pub smoothing: Smoothing,
    pub cover: Cover,
    pub shake: Shake,
    pub tablet: Tablet,
    pub edge_swipes: EdgeSwipes,
    pub areas: Areas,
    pub typing: Typing,
}

impl Config {
    pub fn native_extras(&self) -> bool {
        self.corners.enabled
            || self.rails.enabled
            || self.circular.enabled
            || self.edge_swipes.enabled
            || self.joystick.enabled
            || self.cover.enabled
            || self.shake.enabled
            || self.dwell.enabled
            || self.smoothing.enabled
            || self.edge_motion.enabled
    }

    pub fn uses_typing(&self, native: bool) -> bool {
        self.cover.enabled || (self.typing.enabled && !native)
    }
}

impl From<Settings> for Config {
    fn from(pointer: Settings) -> Self {
        Config { pointer, ..Config::default() }
    }
}
