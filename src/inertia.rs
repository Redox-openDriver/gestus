use crate::geometry::{decay, distance, seconds};

pub(crate) const WINDOW: u64 = 60_000;
pub(crate) const FRESH: u64 = 40_000;
pub(crate) const LIFT_GRACE: u64 = 80_000;
const SAMPLES: usize = 16;
const LEAST_FRICTION: f32 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq, Default)]
struct Sample {
    at: u64,
    span: u64,
    dx: f32,
    dy: f32,
    travel: f32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) struct Recent {
    samples: [Sample; SAMPLES],
    next: usize,
    len: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Velocity {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
}

impl Recent {
    pub fn push(&mut self, at: u64, span: u64, dx: f32, dy: f32, travel: f32) {
        self.samples[self.next] = Sample { at, span: span.min(WINDOW), dx, dy, travel };
        self.next = (self.next + 1) % SAMPLES;
        self.len = (self.len + 1).min(SAMPLES);
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn velocity(&self, now: u64) -> Option<Velocity> {
        let newest = (0..self.len).map(|back| self.samples[(self.next + SAMPLES - 1 - back) % SAMPLES]);
        let latest = newest.clone().next()?;
        if now.saturating_sub(latest.at) > FRESH {
            return None;
        }
        let (mut dx, mut dy, mut travel, mut span) = (0.0f32, 0.0f32, 0.0f32, 0u64);
        for sample in newest.take_while(|sample| latest.at.saturating_sub(sample.at) < WINDOW) {
            dx += sample.dx;
            dy += sample.dy;
            travel += sample.travel;
            span += sample.span;
        }
        if span == 0 {
            return None;
        }
        let time = seconds(span);
        Some(Velocity { x: dx / time, y: dy / time, speed: travel / time })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Glide {
    Scroll,
    Pointer,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Kinetic {
    pub glide: Glide,
    vx: f32,
    vy: f32,
    pub at: u64,
    friction: f32,
    stop: f32,
}

impl Kinetic {
    pub fn new(glide: Glide, velocity: Velocity, at: u64, friction: f32, stop: f32) -> Self {
        Kinetic { glide, vx: velocity.x, vy: velocity.y, at, friction: friction.max(LEAST_FRICTION), stop }
    }

    pub fn step(&mut self, now: u64) -> ((f32, f32), bool) {
        let time = seconds(now.saturating_sub(self.at));
        self.at = now;
        let fade = decay(self.friction, time);
        let carried = (1.0 - fade) / self.friction;
        let delta = (self.vx * carried, self.vy * carried);
        self.vx *= fade;
        self.vy *= fade;
        (delta, distance(self.vx, self.vy) >= self.stop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn velocity_comes_from_the_last_moments_and_a_pause_cancels_it() {
        let mut recent = Recent::default();
        for i in 0..20u64 {
            recent.push(1_000_000 + i * 8_000, 8_000, 4.0, 0.0, 1.0);
        }
        let velocity = recent.velocity(1_160_000).unwrap();
        assert!((velocity.x - 500.0).abs() < 1.0 && velocity.y == 0.0, "{velocity:?}");
        assert!((velocity.speed - 125.0).abs() < 0.5, "{velocity:?}");
        assert_eq!(recent.velocity(1_300_000), None);
        recent.clear();
        assert_eq!(recent.velocity(1_160_000), None);
    }

    #[test]
    fn a_glide_slows_down_by_friction_and_covers_speed_over_friction() {
        let mut glide = Kinetic::new(Glide::Scroll, Velocity { x: 0.0, y: 300.0, speed: 50.0 }, 0, 3.0, 5.0);
        let (mut total, mut at, mut steps) = (0.0f32, 0u64, 0);
        loop {
            at += 8_000;
            let ((_, dy), alive) = glide.step(at);
            total += dy;
            steps += 1;
            if !alive {
                break;
            }
        }
        assert!((total - (300.0 - 5.0) / 3.0).abs() < 2.0, "{total}");
        assert!((150..200).contains(&steps), "{steps}");
    }
}
