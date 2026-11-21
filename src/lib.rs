#![no_std]

extern crate alloc;
#[cfg(feature = "config")]
extern crate std;

#[cfg(feature = "config")]
pub mod config;
mod engine;
mod filter;
mod geometry;
mod inertia;
mod keys;
mod settings;
mod shaper;
mod symbols;
mod taps;
mod zones;

use alloc::vec::Vec;

pub use engine::Gestures;
pub use keys::{Action, COMBO, Combo, Key, Slider, Toggle};
pub use settings::*;
pub use shaper::{Output, Shaper};
pub use symbols::{POINTS, Template, normalize};
pub use zones::{Corner, Edge};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Touch {
    pub id: u8,
    pub x: f32,
    pub y: f32,
    pub palm: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Input {
    pub time: u64,
    pub button: bool,
    pub touches: Vec<Touch>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Surface {
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Begin,
    Update,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notice {
    Touchpad(bool),
    Numpad(bool),
    Tablet(bool),
    Drawing,
    Symbol(u16),
    Unrecognized,
    Cancelled,
}

impl Notice {
    pub const KEYS: [&'static str; 10] = [
        "gesture-touchpad-on",
        "gesture-touchpad-off",
        "gesture-numpad-on",
        "gesture-numpad-off",
        "gesture-tablet-on",
        "gesture-tablet-off",
        "gesture-drawing",
        "gesture-symbol",
        "gesture-unrecognized",
        "gesture-cancelled",
    ];

    pub fn key(self) -> &'static str {
        match self {
            Notice::Touchpad(true) => "gesture-touchpad-on",
            Notice::Touchpad(false) => "gesture-touchpad-off",
            Notice::Numpad(true) => "gesture-numpad-on",
            Notice::Numpad(false) => "gesture-numpad-off",
            Notice::Tablet(true) => "gesture-tablet-on",
            Notice::Tablet(false) => "gesture-tablet-off",
            Notice::Drawing => "gesture-drawing",
            Notice::Symbol(_) => "gesture-symbol",
            Notice::Unrecognized => "gesture-unrecognized",
            Notice::Cancelled => "gesture-cancelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Event {
    Motion { dx: f32, dy: f32 },
    Press(Button),
    Release(Button),
    Scroll { dx: f32, dy: f32 },
    Swipe { fingers: u8, phase: Phase, dx: f32, dy: f32 },
    Key { key: Key, pressed: bool },
    Pen { x: f32, y: f32, down: bool },
    PenAway,
    Touch { fingers: u8, x: f32, y: f32 },
    Notice(Notice),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_notice_has_one_of_the_listed_message_keys_and_each_key_is_used() {
        let notices = [
            Notice::Touchpad(true),
            Notice::Touchpad(false),
            Notice::Numpad(true),
            Notice::Numpad(false),
            Notice::Tablet(true),
            Notice::Tablet(false),
            Notice::Drawing,
            Notice::Symbol(3),
            Notice::Unrecognized,
            Notice::Cancelled,
        ];
        let mut keys: Vec<&str> = notices.iter().map(|notice| notice.key()).collect();
        keys.sort_unstable();
        let mut listed = Notice::KEYS.to_vec();
        listed.sort_unstable();
        assert_eq!(keys, listed);
    }
}
