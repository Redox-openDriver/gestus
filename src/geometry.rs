use core::f32::consts::{PI, TAU};

pub fn distance(dx: f32, dy: f32) -> f32 {
    libm::hypotf(dx, dy)
}

pub fn direction(dx: f32, dy: f32) -> f32 {
    libm::atan2f(dy, dx)
}

pub fn turn(from: f32, to: f32) -> f32 {
    libm::remainderf(to - from, TAU)
}

pub fn degrees(radians: f32) -> f32 {
    radians * 180.0 / PI
}

pub fn decay(rate: f32, seconds: f32) -> f32 {
    libm::expf(-rate * seconds)
}

pub fn seconds(micros: u64) -> f32 {
    micros as f32 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distances_angles_and_turns_behave_like_geometry() {
        for (dx, dy, expected) in [(3.0f32, 4.0f32, 5.0f32), (0.3, 0.4, 0.5), (0.0, 0.0, 0.0), (100.0, 0.0, 100.0)] {
            assert!((distance(dx, dy) - expected).abs() < 1e-5, "{dx} {dy}");
        }
        assert!((degrees(direction(0.0, 1.0)) - 90.0).abs() < 1e-4);
        assert!((degrees(turn(direction(-1.0, -0.01), direction(-1.0, 0.01))) + 1.146).abs() < 0.01);
        assert!((degrees(turn(3.0, -3.0)) - degrees(TAU - 6.0)).abs() < 1e-3);
        assert!((decay(2.0, 0.5) - libm::expf(-1.0)).abs() < 1e-6);
    }
}
