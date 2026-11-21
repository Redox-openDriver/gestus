use core::f32::consts::TAU;

use crate::geometry::seconds;

const SLOPE_CUTOFF: f32 = 1.0;
const LEAST_CUTOFF: f32 = 0.01;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) struct OneEuro {
    value: f32,
    slope: f32,
    at: u64,
    primed: bool,
}

fn alpha(cutoff: f32, time: f32) -> f32 {
    let tau = 1.0 / (TAU * cutoff.max(LEAST_CUTOFF));
    1.0 / (1.0 + tau / time)
}

impl OneEuro {
    pub fn filter(&mut self, value: f32, at: u64, cutoff: f32, beta: f32) -> f32 {
        if !self.primed {
            *self = OneEuro { value, slope: 0.0, at, primed: true };
            return value;
        }
        if at <= self.at {
            return self.value;
        }
        let time = seconds(at - self.at);
        self.at = at;
        let raw = (value - self.value) / time;
        self.slope += alpha(SLOPE_CUTOFF, time) * (raw - self.slope);
        self.value += alpha(cutoff + beta * self.slope.abs(), time) * (value - self.value);
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_is_damped_while_a_steady_move_still_arrives() {
        let mut still = OneEuro::default();
        let mut wobble = 0.0f32;
        let mut previous = still.filter(10.0, 0, 2.0, 0.05);
        for i in 1..100u64 {
            let noisy = 10.0 + if i % 2 == 0 { 0.4 } else { -0.4 };
            let smooth = still.filter(noisy, i * 8_000, 2.0, 0.05);
            wobble += (smooth - previous).abs();
            previous = smooth;
        }
        assert!(wobble < 0.2 * 99.0 * 0.8, "{wobble}");
        let mut moving = OneEuro::default();
        let mut last = 0.0;
        for i in 0..100u64 {
            last = moving.filter(i as f32, i * 8_000, 2.0, 0.05);
        }
        assert!(last > 90.0, "{last}");
    }
}
