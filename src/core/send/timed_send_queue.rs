use std::{collections::HashMap, time};

use seq::Seq16;

use super::{RttStopwatch, SendQueue};

pub struct TimedSendQueue {
    rtt_stopwatches: HashMap<Seq16, RttStopwatch>,
    /// Packets that have been sent but not yet acknowledged
    send_queue: SendQueue,
}

impl TimedSendQueue {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            rtt_stopwatches: HashMap::new(),
            send_queue: SendQueue::new(capacity),
        }
    }

    pub fn cancel_rtt_stopwatch(&mut self, seq: Seq16) {
        self.rtt_stopwatches.remove(&seq);
    }

    pub fn rtt_stopwatch(&self, seq: Seq16) -> Option<&RttStopwatch> {
        self.rtt_stopwatches.get(&seq)
    }

    pub fn send(&mut self, now: time::Instant, timeout: time::Duration) -> Option<Seq16> {
        let Some(seq) = self.send_queue.send() else {
            return None;
        };
        self.rtt_stopwatches
            .insert(seq, RttStopwatch::new(now, timeout));
        Some(seq)
    }

    pub fn ack(&mut self, seq: Seq16, now: time::Instant) -> Option<time::Duration> {
        self.send_queue.ack(seq);
        let rtt_stopwatch = self.rtt_stopwatches.remove(&seq);
        rtt_stopwatch.map(|stopwatch| stopwatch.into_rtt(now))
    }
}
