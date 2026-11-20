//! A short, interruptible transition for deliberate lyric navigation.
use std::time::Instant;

pub(super) struct Jump {
    pub from: f32,
    pub requested: f64,
    pub started: Instant,
}
impl Jump {
    pub fn new(from: f32, requested: f64) -> Self {
        Self {
            from,
            requested,
            started: Instant::now(),
        }
    }
    pub fn sample(from: f32, target: f32, seconds: f32, speed: f32) -> (f32, bool) {
        let duration = (0.22 + (target - from).abs().sqrt() * 0.004).min(0.42);
        let t = (seconds * speed.clamp(0.5, 2.) / duration).clamp(0., 1.);
        // Start and settle gently without overshooting the selected line.
        let ease = t * t * (3. - 2. * t);
        (from + (target - from) * ease, t < 1.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jumps_are_bounded_time_based_and_can_reverse_without_a_position_jump() {
        for target in [0., 400., 20_000.] {
            let mut previous = 250.;
            for step in 0..=120 {
                let (position, _) = Jump::sample(250., target, step as f32 / 120., 1.);
                assert!(
                    position >= 250f32.min(target) - 0.01 && position <= 250f32.max(target) + 0.01
                );
                assert!((position - previous) * (target - 250.) >= -0.01);
                previous = position;
            }
            assert_eq!(Jump::sample(250., target, 0.42, 1.), (target, false));
        }
        let current = Jump::sample(0., 2000., 0.12, 1.).0;
        assert_eq!(Jump::sample(current, 0., 0., 1.).0, current);
        for fps in [30, 60, 120, 240] {
            let (end, moving) = Jump::sample(0., 2000., fps as f32 / fps as f32, 1.);
            assert_eq!(end, 2000.);
            assert!(!moving);
        }
    }
}
