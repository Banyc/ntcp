use std::{collections::HashMap, time};

use seq::Seq16;

use super::{RttStopwatch, SendQueue};

pub struct RetransmitQueue {
    rtt_stopwatches: HashMap<Seq16, RttStopwatch>,
    /// Packets that have been sent but not yet acknowledged
    send_queue: SendQueue,
}

impl RetransmitQueue {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            rtt_stopwatches: HashMap::new(),
            send_queue: SendQueue::new(capacity),
        }
    }

    #[must_use]
    pub fn retransmit(
        &mut self,
        seq: Seq16,
        now: time::Instant,
        next_timeout: time::Duration,
    ) -> Result<RetransmitResult, RetransmitError> {
        let Some(rtt_stopwatch) = self.rtt_stopwatches.get(&seq) else {
            return Err(RetransmitError::SequenceNumberNotFound);
        };
        if !rtt_stopwatch.has_timed_out(now) {
            return Ok(RetransmitResult::Wait);
        }
        self.rtt_stopwatches.remove(&seq);
        self.rtt_stopwatches
            .insert(seq, RttStopwatch::new(now, next_timeout));
        Ok(RetransmitResult::Retransmit)
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum RetransmitResult {
    Wait,
    Retransmit,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum RetransmitError {
    SequenceNumberNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok() {
        let mut queue = RetransmitQueue::new(10);
        let now = time::Instant::now();
        let timeout = time::Duration::from_millis(100);
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout),
            Err(RetransmitError::SequenceNumberNotFound)
        );
        assert_eq!(queue.send(now, timeout), Some(Seq16::new(0)));
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout),
            Ok(RetransmitResult::Wait)
        );
        assert_eq!(
            queue.retransmit(Seq16::new(1), now, timeout),
            Err(RetransmitError::SequenceNumberNotFound)
        );
        let now = now + timeout;
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout),
            Ok(RetransmitResult::Retransmit)
        );
        assert_eq!(
            queue.ack(Seq16::new(0), now),
            Some(time::Duration::from_secs(0))
        );
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout),
            Err(RetransmitError::SequenceNumberNotFound)
        );
    }
}
