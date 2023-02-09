use std::time;

pub struct RttStopwatch {
    start: time::Instant,
    timeout: time::Duration,
}

impl RttStopwatch {
    pub fn new(now: time::Instant, timeout: time::Duration) -> Self {
        Self {
            start: now,
            timeout,
        }
    }

    pub fn is_timeout(&self, now: time::Instant) -> bool {
        now - self.start >= self.timeout
    }

    pub fn into_rtt(self, now: time::Instant) -> time::Duration {
        now - self.start
    }

    pub fn timeout(&self) -> time::Duration {
        self.timeout
    }
}
