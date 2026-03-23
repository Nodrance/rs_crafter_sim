use std::time::{Duration, Instant};

/// A periodic logger that emits debug messages at regular intervals to track progress.
/// Use this to pass around state tracking without manual interval/timestamp management.
pub struct PeriodicLogger {
    interval: Duration,
    last_log_at: Instant,
}

impl PeriodicLogger {
    /// Creates a new periodic logger with the given heartbeat interval.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_log_at: Instant::now(),
        }
    }

    /// Logs a message if enough time has elapsed since the last log, resetting the timer.
    /// Returns true if the message was logged, false if the interval hasn't elapsed.
    pub fn heartbeat(&mut self, message: &str) -> bool {
        if self.last_log_at.elapsed() >= self.interval {
            crate::debugln!("{}", message);
            self.last_log_at = Instant::now();
            true
        } else {
            false
        }
    }

    /// Logs a message unconditionally and resets the progress timer.
    pub fn immediate(&mut self, message: &str) {
        crate::debugln!("{}", message);
        self.last_log_at = Instant::now();
    }

    /// Resets the progress timer without logging anything.
    pub fn reset(&mut self) {
        self.last_log_at = Instant::now();
    }

    /// Returns the elapsed time since the last log.
    pub fn elapsed_since_last_log(&self) -> Duration {
        self.last_log_at.elapsed()
    }
}
