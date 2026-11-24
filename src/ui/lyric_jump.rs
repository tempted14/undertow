//! A short, interruptible transition for deliberate lyric navigation.
use std::time::Instant;

pub(super) struct Jump {
    from: f32,
    velocity: f32,
    pub requested: f64,
    pub started: Instant,
}
impl Jump {
    pub fn new(from: f32, velocity: f32, requested: f64) -> Self {
        Self {
            from,
            velocity,
            requested,
            started: Instant::now(),
        }
    }
    pub fn sample(&self, target: f32, seconds: f32, speed: f32) -> (f32, f32, bool) {
        let distance = target - self.from;
        let duration = (0.20 + distance.abs().sqrt() * 0.007).min(0.60) / speed.clamp(0.5, 2.);
        let t = (seconds / duration).clamp(0., 1.);
        if t >= 1. || distance.abs() < 0.02 {
            return (target, 0., false);
        }
        // Carry forward momentum from a tap in the same direction. Clamp the
        // tangent so short or reversed jumps stay between their endpoints.
        let tangent = (self.velocity * duration / distance).clamp(0., 2.5);
        let progress = t * (tangent + t * (3. - 2. * tangent + t * (tangent - 2.)));
        let velocity = distance / duration * (1. - t) * (tangent + (6. - 3. * tangent) * t);
        (self.from + distance * progress, velocity, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jumps_are_bounded_time_based_and_can_reverse_without_a_position_jump() {
        for target in [0., 400., 20_000.] {
            let jump = Jump::new(250., 0., 0.);
            let mut previous = 250.;
            for step in 0..=120 {
                let (position, _, _) = jump.sample(target, step as f32 / 120., 1.);
                assert!(
                    position >= 250f32.min(target) - 0.01 && position <= 250f32.max(target) + 0.01
                );
                assert!((position - previous) * (target - 250.) >= -0.01);
                previous = position;
            }
            assert_eq!(jump.sample(target, 0.60, 1.), (target, 0., false));
        }
        let jump = Jump::new(0., 0., 0.);
        let (current, velocity, _) = jump.sample(400., 0.12, 1.);
        let redirected = Jump::new(current, velocity, 0.);
        let (next_position, next_velocity, _) = redirected.sample(900., 0., 1.);
        assert_eq!(next_position, current);
        assert!((next_velocity - velocity).abs() < 0.01);
        assert_eq!(redirected.sample(0., 0., 1.), (current, 0., true));
        for fps in [30, 60, 120, 240] {
            let (end, _, moving) = jump.sample(2000., fps as f32 / fps as f32, 1.);
            assert_eq!(end, 2000.);
            assert!(!moving);
        }
    }
}
