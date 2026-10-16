use std::time::{Duration, Instant};

pub const TAP_MAX: Duration = Duration::from_millis(300);
pub const GESTURE_MAX: Duration = Duration::from_millis(750);

#[derive(Default)]
pub struct DoubleShift {
    down_at: Option<Instant>,
    first_down: Option<Instant>,
}

impl DoubleShift {
    pub fn shift(&mut self, down: bool, at: Instant) -> bool {
        if down {
            self.down_at.get_or_insert(at);
            return false;
        }
        let Some(pressed) = self.down_at.take() else { return false };
        if at.saturating_duration_since(pressed) > TAP_MAX {
            self.first_down = None;
            return false;
        }
        match self.first_down.take() {
            Some(first) if at.saturating_duration_since(first) <= GESTURE_MAX => true,
            _ => {
                self.first_down = Some(pressed);
                false
            }
        }
    }

    pub fn other(&mut self) {
        self.down_at = None;
        self.first_down = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(t0: Instant, n: u64) -> Instant {
        t0 + Duration::from_millis(n)
    }

    #[test]
    fn double_tap_fires() {
        let t = Instant::now();
        let mut g = DoubleShift::default();
        assert!(!g.shift(true, ms(t, 0)));
        assert!(!g.shift(true, ms(t, 30)));
        assert!(!g.shift(false, ms(t, 80)));
        assert!(!g.shift(true, ms(t, 200)));
        assert!(g.shift(false, ms(t, 260)));
        assert!(!g.shift(true, ms(t, 300)));
        assert!(!g.shift(false, ms(t, 350)));
    }

    #[test]
    fn slow_or_interrupted_does_not_fire() {
        let t = Instant::now();
        let mut g = DoubleShift::default();
        g.shift(true, ms(t, 0));
        g.shift(false, ms(t, 400));
        g.shift(true, ms(t, 450));
        assert!(!g.shift(false, ms(t, 500)));

        let mut g = DoubleShift::default();
        g.shift(true, ms(t, 0));
        g.shift(false, ms(t, 50));
        g.shift(true, ms(t, 700));
        assert!(!g.shift(false, ms(t, 800)));

        let mut g = DoubleShift::default();
        g.shift(true, ms(t, 0));
        g.shift(false, ms(t, 50));
        g.other();
        g.shift(true, ms(t, 100));
        assert!(!g.shift(false, ms(t, 150)));
    }
}
