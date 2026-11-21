use alloc::vec;
use alloc::vec::Vec;

use crate::Surface;
use crate::keys::Action;
use crate::settings::Config;
use crate::zones::{Corner, Zone};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Tap {
    pub fingers: u8,
    pub x: f32,
    pub y: f32,
    pub at: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct Pattern {
    zones: Vec<Zone>,
    action: Action,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Patterns {
    list: Vec<Pattern>,
    surface: Surface,
    size: f32,
}

impl Patterns {
    pub fn new(config: &Config, surface: Surface, knock: bool) -> Self {
        let mut list = Vec::new();
        if config.corners.enabled {
            for corner in Corner::ALL {
                let actions = config.corners.get(corner);
                if let Some(action) = actions.tap {
                    list.push(Pattern { zones: vec![Zone::Corner(corner)], action });
                }
                if let Some(action) = actions.double_tap {
                    list.push(Pattern { zones: vec![Zone::Corner(corner); 2], action });
                }
            }
        }
        if knock && config.knock.enabled {
            for (code, action) in &config.knock.codes {
                list.push(Pattern { zones: code.iter().map(|quadrant| Zone::Quadrant(*quadrant)).collect(), action: *action });
            }
        }
        Patterns { list, surface, size: config.corners.size }
    }

    fn fits(&self, pattern: &Pattern, taps: &[Tap]) -> bool {
        taps.len() <= pattern.zones.len()
            && taps.iter().zip(&pattern.zones).all(|(tap, zone)| tap.fingers == 1 && zone.contains(tap.x, tap.y, self.surface, self.size))
    }

    pub fn starts(&self, taps: &[Tap], allow: impl Fn(&Action) -> bool) -> bool {
        self.list.iter().any(|pattern| allow(&pattern.action) && self.fits(pattern, taps))
    }

    pub fn longer(&self, taps: &[Tap], allow: impl Fn(&Action) -> bool) -> bool {
        self.list.iter().any(|pattern| allow(&pattern.action) && pattern.zones.len() > taps.len() && self.fits(pattern, taps))
    }

    pub fn exact(&self, taps: &[Tap], allow: impl Fn(&Action) -> bool) -> Option<Action> {
        self.list.iter().find(|pattern| allow(&pattern.action) && pattern.zones.len() == taps.len() && self.fits(pattern, taps)).map(|pattern| pattern.action)
    }

    pub fn continues(&self, taps: &[Tap], x: f32, y: f32, allow: impl Fn(&Action) -> bool) -> bool {
        self.list.iter().any(|pattern| {
            allow(&pattern.action)
                && pattern.zones.len() > taps.len()
                && self.fits(pattern, taps)
                && pattern.zones[taps.len()].contains(x, y, self.surface, self.size)
        })
    }

    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }
}
