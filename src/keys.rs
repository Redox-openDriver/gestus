use crate::Button;

macro_rules! keys {
    ($($key:ident = $name:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub enum Key {
            $($key),*
        }

        impl Key {
            pub const ALL: &'static [Key] = &[$(Key::$key),*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Key::$key => $name),*
                }
            }
        }
    };
}

keys! {
    A = "a", B = "b", C = "c", D = "d", E = "e", F = "f", G = "g", H = "h", I = "i", J = "j", K = "k", L = "l", M = "m",
    N = "n", O = "o", P = "p", Q = "q", R = "r", S = "s", T = "t", U = "u", V = "v", W = "w", X = "x", Y = "y", Z = "z",
    Digit0 = "0", Digit1 = "1", Digit2 = "2", Digit3 = "3", Digit4 = "4",
    Digit5 = "5", Digit6 = "6", Digit7 = "7", Digit8 = "8", Digit9 = "9",
    F1 = "f1", F2 = "f2", F3 = "f3", F4 = "f4", F5 = "f5", F6 = "f6", F7 = "f7", F8 = "f8", F9 = "f9", F10 = "f10", F11 = "f11", F12 = "f12",
    Escape = "escape", Tab = "tab", Enter = "enter", Space = "space", Backspace = "backspace", Delete = "delete", Insert = "insert",
    Home = "home", End = "end", PageUp = "page-up", PageDown = "page-down", Left = "left", Right = "right", Up = "up", Down = "down",
    Minus = "minus", Equal = "equal", Comma = "comma", Dot = "dot", Slash = "slash", Semicolon = "semicolon", Apostrophe = "apostrophe",
    Grave = "grave", BracketLeft = "bracket-left", BracketRight = "bracket-right", Backslash = "backslash",
    Ctrl = "ctrl", Shift = "shift", Alt = "alt", Super = "super", Menu = "menu",
    Kp0 = "kp-0", Kp1 = "kp-1", Kp2 = "kp-2", Kp3 = "kp-3", Kp4 = "kp-4", Kp5 = "kp-5", Kp6 = "kp-6", Kp7 = "kp-7", Kp8 = "kp-8", Kp9 = "kp-9",
    KpDot = "kp-dot", KpPlus = "kp-plus", KpMinus = "kp-minus", KpAsterisk = "kp-asterisk", KpSlash = "kp-slash", KpEnter = "kp-enter",
    VolumeUp = "volume-up", VolumeDown = "volume-down", Mute = "mute", MicMute = "mic-mute",
    PlayPause = "play-pause", NextTrack = "next-track", PreviousTrack = "previous-track", StopMedia = "stop-media",
    BrightnessUp = "brightness-up", BrightnessDown = "brightness-down",
    Print = "print", Calculator = "calculator", Browser = "browser", Mail = "mail", Files = "files", Search = "search", LockScreen = "lock-screen",
}

const ALIASES: [(&str, Key); 10] = [
    ("control", Key::Ctrl),
    ("win", Key::Super),
    ("meta", Key::Super),
    ("esc", Key::Escape),
    ("return", Key::Enter),
    ("del", Key::Delete),
    ("period", Key::Dot),
    ("screenshot", Key::Print),
    ("calc", Key::Calculator),
    ("lock", Key::LockScreen),
];

impl Key {
    pub fn from_name(name: &str) -> Option<Key> {
        let name = name.trim();
        Key::ALL
            .iter()
            .copied()
            .find(|key| key.name().eq_ignore_ascii_case(name))
            .or_else(|| ALIASES.iter().find(|(alias, _)| alias.eq_ignore_ascii_case(name)).map(|(_, key)| *key))
    }
}

pub const COMBO: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Combo {
    keys: [Option<Key>; COMBO],
}

impl Combo {
    pub fn of(keys: &[Key]) -> Option<Combo> {
        if keys.is_empty() || keys.len() > COMBO {
            return None;
        }
        let mut combo = Combo { keys: [None; COMBO] };
        for (slot, key) in combo.keys.iter_mut().zip(keys) {
            *slot = Some(*key);
        }
        Some(combo)
    }

    pub fn parse(text: &str) -> Option<Combo> {
        let mut keys = [Key::Ctrl; COMBO];
        let mut count = 0;
        for part in text.split('+') {
            let key = Key::from_name(part)?;
            *keys.get_mut(count)? = key;
            count += 1;
        }
        Combo::of(&keys[..count])
    }

    pub fn keys(&self) -> impl DoubleEndedIterator<Item = Key> + '_ {
        self.keys.iter().flatten().copied()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Toggle {
    Touchpad,
    Numpad,
    Tablet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Keys(Combo),
    Click(Button),
    Toggle(Toggle),
    Draw,
}

const PRESETS: [(&str, &str); 24] = [
    ("back", "alt+left"),
    ("forward", "alt+right"),
    ("copy", "ctrl+c"),
    ("paste", "ctrl+v"),
    ("cut", "ctrl+x"),
    ("undo", "ctrl+z"),
    ("redo", "ctrl+shift+z"),
    ("save", "ctrl+s"),
    ("find", "ctrl+f"),
    ("close", "ctrl+w"),
    ("new-tab", "ctrl+t"),
    ("reopen-tab", "ctrl+shift+t"),
    ("next-tab", "ctrl+tab"),
    ("previous-tab", "ctrl+shift+tab"),
    ("refresh", "f5"),
    ("fullscreen", "f11"),
    ("overview", "super"),
    ("switch-window", "alt+tab"),
    ("zoom-in", "ctrl+kp-plus"),
    ("zoom-out", "ctrl+kp-minus"),
    ("zoom-reset", "ctrl+0"),
    ("locate", "ctrl"),
    ("rotate-right", "ctrl+r"),
    ("rotate-left", "ctrl+shift+r"),
];

impl Action {
    pub fn parse(text: &str) -> Option<Action> {
        let text = text.trim();
        match text {
            "left-click" => Some(Action::Click(Button::Left)),
            "right-click" => Some(Action::Click(Button::Right)),
            "middle-click" => Some(Action::Click(Button::Middle)),
            "toggle-touchpad" => Some(Action::Toggle(Toggle::Touchpad)),
            "toggle-numpad" | "numpad" => Some(Action::Toggle(Toggle::Numpad)),
            "toggle-tablet" | "tablet" => Some(Action::Toggle(Toggle::Tablet)),
            "draw" => Some(Action::Draw),
            _ => {
                let combo = PRESETS.iter().find(|(name, _)| *name == text).map_or(text, |(_, combo)| combo);
                Combo::parse(combo).map(Action::Keys)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Slider {
    #[default]
    Off,
    Scroll,
    Steps {
        forward: Action,
        back: Action,
    },
}

impl Slider {
    pub fn pair(forward: &str, back: &str) -> Option<Slider> {
        Some(Slider::Steps { forward: Action::parse(forward)?, back: Action::parse(back)? })
    }

    pub fn preset(name: &str) -> Option<Slider> {
        match name.trim() {
            "none" | "off" => Some(Slider::Off),
            "scroll" => Some(Slider::Scroll),
            "volume" => Slider::pair("volume-up", "volume-down"),
            "brightness" => Slider::pair("brightness-up", "brightness-down"),
            "zoom" => Slider::pair("zoom-in", "zoom-out"),
            "tabs" => Slider::pair("next-tab", "previous-tab"),
            "history" => Slider::pair("redo", "undo"),
            "tracks" => Slider::pair("next-track", "previous-track"),
            "rotate" => Slider::pair("rotate-right", "rotate-left"),
            _ => None,
        }
    }
}

const MOST_STEPS: f32 = 16.0;

pub fn steps(accumulated: f32, delta: f32, step: f32) -> (i32, f32) {
    if !step.is_finite() || step <= 0.0 {
        return (0, accumulated);
    }
    let total = accumulated + delta;
    if !total.is_finite() {
        return (0, 0.0);
    }
    let raw = total / step;
    if raw.abs() > MOST_STEPS {
        return ((MOST_STEPS * raw.signum()) as i32, 0.0);
    }
    let whole = raw as i32;
    (whole, total - whole as f32 * step)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    #[test]
    fn a_tiny_or_broken_step_never_floods_a_frame() {
        assert_eq!(steps(0.0, 5.0, 1e-30), (16, 0.0));
        assert_eq!(steps(0.0, -5.0, 1e-30), (-16, 0.0));
        assert_eq!(steps(0.5, 2.0, 1.0), (2, 0.5));
        assert_eq!(steps(0.0, f32::NAN, 1.0), (0, 0.0));
        assert_eq!(steps(0.25, 1.0, f32::NAN), (0, 0.25));
    }

    #[test]
    fn keys_are_found_by_name_and_alias_and_every_name_is_unique() {
        assert_eq!(Key::from_name("Ctrl"), Some(Key::Ctrl));
        assert_eq!(Key::from_name("win"), Some(Key::Super));
        assert_eq!(Key::from_name("kp-plus"), Some(Key::KpPlus));
        assert_eq!(Key::from_name("7"), Some(Key::Digit7));
        assert_eq!(Key::from_name("hyper"), None);
        let mut names: Vec<&str> = Key::ALL.iter().map(|key| key.name()).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Key::ALL.len());
    }

    #[test]
    fn combinations_keep_their_order_and_have_a_limit() {
        let combo = Combo::parse("ctrl + shift+t").unwrap();
        assert_eq!(combo.keys().collect::<Vec<_>>(), [Key::Ctrl, Key::Shift, Key::T]);
        assert_eq!(combo.keys().rev().collect::<Vec<_>>(), [Key::T, Key::Shift, Key::Ctrl]);
        assert_eq!(Combo::parse("ctrl+alt+shift+super+t"), None);
        assert_eq!(Combo::parse("ctrl+"), None);
        assert_eq!(Combo::parse(""), None);
    }

    #[test]
    fn actions_are_clicks_toggles_presets_or_key_combinations() {
        assert_eq!(Action::parse("right-click"), Some(Action::Click(Button::Right)));
        assert_eq!(Action::parse("toggle-touchpad"), Some(Action::Toggle(Toggle::Touchpad)));
        assert_eq!(Action::parse("back"), Action::parse("alt+left"));
        assert_eq!(Action::parse("volume-up"), Some(Action::Keys(Combo::of(&[Key::VolumeUp]).unwrap())));
        assert_eq!(Action::parse("dance"), None);
        assert_eq!(Slider::preset("volume"), Slider::pair("volume-up", "volume-down"));
        assert_eq!(Slider::preset("scroll"), Some(Slider::Scroll));
        assert_eq!(Slider::preset("loud"), None);
    }

    #[test]
    fn sliders_step_by_whole_amounts_and_keep_the_rest() {
        assert_eq!(steps(0.0, 9.0, 4.0), (2, 1.0));
        assert_eq!(steps(1.0, -6.0, 4.0), (-1, -1.0));
        assert_eq!(steps(0.5, 0.2, 0.0), (0, 0.5));
    }
}
