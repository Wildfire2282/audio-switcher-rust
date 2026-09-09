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
        Self {
            history: VecDeque::new(),
        }
    }

    /// Push a wheel tick at `now`, returning the step percent `1`, `2`, or `5`.
    ///
    /// Acceleration is always on: fast scrolling yields larger steps.
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
    pub fn push(&mut self, now: Instant, delta: i32) -> u32 {
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
        calc_step(effective_count, last_interval.as_millis())
    }

    /// Convert a raw `delta` plus per-tick `step` into a signed volume delta.
    ///
    /// `unsigned_abs` keeps `i32::MIN` representable and the `i64` product
    /// cannot overflow (`u32 × i32` always fits); the result clamps to the
    /// `i32` range so a huge public `step_per_tick` saturates instead of
    /// wrapping the wheel the wrong way. A sub-notch tick still steps once.
    #[must_use]
    pub fn total_step(delta: i32, step_per_tick: u32) -> i32 {
        let sign: i64 = if delta >= 0 { 1 } else { -1 };
        let step = i64::from(i32::try_from(step_per_tick).unwrap_or(1));
        let ticks = i64::from(delta.unsigned_abs() / 120);
        let magnitude = if ticks == 0 {
            step
        } else {
            ticks.saturating_mul(step)
        };
        (sign * magnitude.min(i64::from(i32::MAX))) as i32
    }

    /// Clear history (e.g. on hover leave).
    pub fn clear(&mut self) {
        self.history.clear();
    }
}

/// Compute step from history size and minimal interval.
///
/// Acceleration is always on: bursts (`count >= 5`) or fast intervals
/// (`< 80ms`) step 5%, moderate bursts step 2%, isolated ticks step 1%.
#[must_use]
pub fn calc_step(count: usize, min_interval_ms: u128) -> u32 {
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
        assert_eq!(calc_step(1, 200), 1);
        assert_eq!(calc_step(3, 100), 2);
        assert_eq!(calc_step(5, 100), 5);
        assert_eq!(calc_step(3, 50), 5);
    }

    #[test]
    fn wheel_state_progression() {
        let mut ws = WheelState::new();
        let base = Instant::now();
        let s1 = ws.push(base, 120);
        assert_eq!(s1, 1);
        let s2 = ws.push(base + Duration::from_millis(50), 120);
        assert_eq!(s2, 5);
        let s3 = ws.push(base + Duration::from_millis(100), 120);
        assert_eq!(s3, 5);
        let mut ws2 = WheelState::new();
        let b = Instant::now();
        assert_eq!(ws2.push(b, 120), 1);
        assert_eq!(ws2.push(b + Duration::from_millis(90), 120), 1);
        assert_eq!(ws2.push(b + Duration::from_millis(180), 120), 2);
        assert_eq!(ws2.push(b + Duration::from_millis(270), 120), 2);
        let mut ws3 = WheelState::new();
        let s = ws3.push(b, 240);
        // 240 is 2 ticks but still single event — effective_count=2, still 1
        assert_eq!(s, 1);
    }

    #[test]
    fn wheel_i32_min() {
        let mut ws = WheelState::new();
        let b = Instant::now();
        // Should not panic
        let s = ws.push(b, i32::MIN);
        assert!(s == 1 || s == 2 || s == 5);
        assert_eq!(WheelState::total_step(i32::MIN, 1), i32::MIN / 120);
    }

    #[test]
    fn wheel_large_delta() {
        let mut ws = WheelState::new();
        let b = Instant::now();
        let s = ws.push(b, 480);
        // 480 = 4 ticks, effective_count = 1 + 3 =4 -> step 2
        assert_eq!(s, 2);
    }

    #[test]
    fn total_step_sign_and_scaling() {
        // Single tick keeps sign with per-tick step.
        assert_eq!(WheelState::total_step(120, 1), 1);
        assert_eq!(WheelState::total_step(-120, 1), -1);
        // Sub-tick delta still yields one signed step.
        assert_eq!(WheelState::total_step(60, 2), 2);
        assert_eq!(WheelState::total_step(-60, 2), -2);
        // Multi-tick scales linearly with sign.
        assert_eq!(WheelState::total_step(240, 2), 4);
        assert_eq!(WheelState::total_step(-240, 5), -10);
    }
}
