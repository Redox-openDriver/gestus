use crate::keys::Key;
use crate::{Button, Event};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Move { dx: i32, dy: i32 },
    Buttons { left: bool, middle: bool, right: bool },
    Wheel { x: i32, y: i32 },
    Key { key: Key, pressed: bool },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Shaper {
    carry: (f32, f32),
    wheel: (f32, f32),
    left: bool,
    middle: bool,
    right: bool,
}

fn whole(carry: &mut f32, delta: f32) -> i32 {
    let total = *carry + delta;
    let step = total as i32;
    *carry = total - step as f32;
    step
}

impl Shaper {
    pub fn shape(&mut self, event: Event, units_per_notch: f32) -> Option<Output> {
        match event {
            Event::Motion { dx, dy } => {
                let (x, y) = (whole(&mut self.carry.0, dx), whole(&mut self.carry.1, dy));
                (x != 0 || y != 0).then_some(Output::Move { dx: x, dy: y })
            }
            Event::Press(button) | Event::Release(button) => {
                let pressed = matches!(event, Event::Press(_));
                match button {
                    Button::Left => self.left = pressed,
                    Button::Middle => self.middle = pressed,
                    Button::Right => self.right = pressed,
                }
                Some(Output::Buttons { left: self.left, middle: self.middle, right: self.right })
            }
            Event::Scroll { dx, dy } => {
                let x = whole(&mut self.wheel.0, dx / units_per_notch);
                let y = whole(&mut self.wheel.1, -dy / units_per_notch);
                (x != 0 || y != 0).then_some(Output::Wheel { x, y })
            }
            Event::Key { key, pressed } => Some(Output::Key { key, pressed }),
            Event::Swipe { .. } | Event::Pen { .. } | Event::PenAway | Event::Touch { .. } | Event::Notice(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Phase;
    use alloc::vec::Vec;

    #[test]
    fn the_shaper_turns_fractions_into_whole_steps_and_keeps_the_button_state() {
        let mut shaper = Shaper::default();
        let moves: Vec<Option<Output>> = [0.4f32, 0.4, 0.4].iter().map(|dx| shaper.shape(Event::Motion { dx: *dx, dy: 0.0 }, 30.0)).collect();
        assert_eq!(moves, [None, None, Some(Output::Move { dx: 1, dy: 0 })]);
        assert_eq!(shaper.shape(Event::Press(Button::Left), 30.0), Some(Output::Buttons { left: true, middle: false, right: false }));
        assert_eq!(shaper.shape(Event::Press(Button::Right), 30.0), Some(Output::Buttons { left: true, middle: false, right: true }));
        assert_eq!(shaper.shape(Event::Release(Button::Left), 30.0), Some(Output::Buttons { left: false, middle: false, right: true }));
        assert_eq!(shaper.shape(Event::Scroll { dx: 0.0, dy: 20.0 }, 30.0), None);
        assert_eq!(shaper.shape(Event::Scroll { dx: 0.0, dy: 20.0 }, 30.0), Some(Output::Wheel { x: 0, y: -1 }));
        assert_eq!(shaper.shape(Event::Swipe { fingers: 3, phase: Phase::Begin, dx: 1.0, dy: 0.0 }, 30.0), None);
        assert_eq!(shaper.shape(Event::Key { key: Key::Mute, pressed: true }, 30.0), Some(Output::Key { key: Key::Mute, pressed: true }));
    }
}
