//! Wheel acceleration — converts scroll events into volume steps.
//!
//! Fast scrolling yields larger steps (1% → 2% → 5%).

use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Tracks recent wheel events to compute acceleration.
#[derive(Debug, Default)]
pub struct WheelState {
    history: VecDeque<Instant>,
}

impl WheelState {
    /// Create an empty state.
    #[must_use]
    pub fn new() -> Self {
        Self { history: VecDeque::new() }
    }

    /// Push a wheel tick at `now`, returning the step percent `1`, `2`, or `5`.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn push(&mut self, now: Instant, wheel_accel: bool, delta: i32) -> u32 {
        if !wheel_accel {
            return 1;
        }
        // Handle i32::MIN without panic; saturate to MAX magnitude.
        let abs = delta.checked_abs().unwrap_or(i32::MAX) as u32;
        let ticks = usize::try_from((abs / 120).max(1)).unwrap_or(1);
        // Evict entries older than 200ms window.
        while let Some(front) = self.history.front() {
            if now.duration_since(*front) > Duration::from_millis(200) {
                self.history.pop_front();
            } else {
                break;
            }
        }
        // Push once per physical event, not per tick, to avoid artificially
        // inflating count and forcing step=5 on large deltas. Scale via total_step instead.
        self.history.push_back(now);
        let count = self.history.len();
        // For large deltas, treat as count + ticks factor for acceleration.
        let effective_count = count + ticks.saturating_sub(1);
        let last_interval = if count >= 2 {
            self.history[count - 1].duration_since(self.history[count - 2])
        } else {
            Duration::from_millis(200)
        };
        calc_step(effective_count, last_interval.as_millis(), wheel_accel)
    }

    /// Convert a raw `delta` plus per-tick `step` into a signed volume delta.
    #[must_use]
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_wrap, clippy::cast_possible_truncation)]
    pub fn total_step(delta: i32, step_per_tick: u32) -> i32 {
        // Handle i32::MIN correctly.
        let abs = delta.checked_abs().unwrap_or(i32::MAX) as u32;
        let ticks = (abs / 120) as i32;
        let step = i32::try_from(step_per_tick).unwrap_or(1);
        let sign = if delta >= 0 { 1 } else { -1 };
        if ticks == 0 {
            sign * step
        } else {
            // Large delta scaling already accounts for ticks; step is per-tick.
            ticks * step * sign
        }
    }

    /// Clear history (e.g. on hover leave).
    pub fn clear(&mut self) {
        self.history.clear();
    }
}

/// Compute step from history size and minimal interval.
#[must_use]
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
        // 240 is 2 ticks but still single event — effective_count=2, still 1
        assert_eq!(s, 1);
    }

    #[test]
    fn wheel_i32_min() {
        let mut ws = WheelState::new();
        let b = Instant::now();
        // Should not panic
        let s = ws.push(b, true, i32::MIN);
        assert!(s == 1 || s == 2 || s == 5);
        assert_eq!(WheelState::total_step(i32::MIN, 1), i32::MIN / 120);
    }

    #[test]
    fn wheel_large_delta() {
        let mut ws = WheelState::new();
        let b = Instant::now();
        let s = ws.push(b, true, 480);
        // 480 = 4 ticks, effective_count = 1 + 3 =4 -> step 2
        assert_eq!(s, 2);
    }
}
