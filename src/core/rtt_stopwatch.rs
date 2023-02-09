use std::time;

pub struct RttStopwatch {
    start: time::Instant,
    timeout: time::Duration,
}

impl RttStopwatch {
    #[must_use]
    pub fn new(now: time::Instant, timeout: time::Duration) -> Self {
        Self {
            start: now,
            timeout,
        }
    }

    #[must_use]
    pub fn is_timeout(&self, now: time::Instant) -> bool {
        now - self.start >= self.timeout
    }

    #[must_use]
    pub fn into_rtt(self, now: time::Instant) -> time::Duration {
        now - self.start
    }

    #[must_use]
    pub fn timeout(&self) -> time::Duration {
        self.timeout
    }
}
