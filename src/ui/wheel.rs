use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
pub struct WheelState {
    history: VecDeque<Instant>,
}

impl WheelState {
    pub fn new() -> Self {
        Self {
            history: VecDeque::new(),
        }
    }

    /// push a wheel tick at now, returns step percent (1,2,5)
    pub fn push(&mut self, now: Instant, wheel_accel: bool, delta: i32) -> u32 {
        if !wheel_accel {
            return 1;
        }
        let ticks = (delta.abs() / 120).max(1) as usize;
        while let Some(front) = self.history.front() {
            if now.duration_since(*front) > Duration::from_millis(200) {
                self.history.pop_front();
            } else {
                break;
            }
        }
        for _ in 0..ticks {
            self.history.push_back(now);
        }
        let count = self.history.len();
        let last_interval = if count >= 2 {
            self.history[count - 1].duration_since(self.history[count - 2])
        } else {
            Duration::from_millis(200)
        };
        calc_step(count, last_interval.as_millis(), wheel_accel)
    }

    pub fn total_step(delta: i32, step_per_tick: u32) -> i32 {
        let ticks = delta / 120;
        if ticks == 0 {
            if delta > 0 {
                step_per_tick as i32
            } else {
                -(step_per_tick as i32)
            }
        } else {
            ticks * step_per_tick as i32
        }
    }

    pub fn clear(&mut self) {
        self.history.clear();
    }
}

pub fn calc_step(count: usize, min_interval_ms: u128, wheel_accel: bool) -> u32 {
    if !wheel_accel {
        return 1;
    }
    if count >= 5 || min_interval_ms < 80 {
        5
    } else if count >= 3 {
        2
    } else {
        1
    }
}

// backwards compat alias used by tray::tests if needed
#[allow(unused_imports)]
pub use calc_step as calc_wheel_step;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wheel_calc() {
        assert_eq!(calc_step(1, 200, true), 1);
        assert_eq!(calc_step(3, 100, true), 2);
        assert_eq!(calc_step(5, 100, true), 5);
        assert_eq!(calc_step(3, 50, true), 5);
        assert_eq!(calc_step(5, 50, false), 1);
    }
    #[test]
    fn wheel_state_progression() {
        let mut ws = WheelState::new();
        let base = Instant::now();
        let s1 = ws.push(base, true, 120);
        assert_eq!(s1, 1);
        let s2 = ws.push(base + Duration::from_millis(50), true, 120);
        assert_eq!(s2, 5);
        let s3 = ws.push(base + Duration::from_millis(100), true, 120);
        assert_eq!(s3, 5);
        let mut ws2 = WheelState::new();
        let b = Instant::now();
        assert_eq!(ws2.push(b, true, 120), 1);
        assert_eq!(ws2.push(b + Duration::from_millis(90), true, 120), 1);
        assert_eq!(ws2.push(b + Duration::from_millis(180), true, 120), 2);
        assert_eq!(ws2.push(b + Duration::from_millis(270), true, 120), 2);
        let mut ws3 = WheelState::new();
        let s = ws3.push(b, true, 240);
        assert_eq!(s, 5);
    }
}
