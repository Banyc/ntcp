use std::{collections::BTreeMap, time};

use seq::Seq16;

use super::{RttStopwatch, SendQueue};

pub struct TimedSendQueue {
    rtt_stopwatches: BTreeMap<Seq16, RttStopwatch2>,
    /// Packets that have been sent but not yet acknowledged
    send_queue: SendQueue,
}

impl TimedSendQueue {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            rtt_stopwatches: BTreeMap::new(),
            send_queue: SendQueue::new(capacity),
        }
    }

    pub fn retransmit(
        &mut self,
        seq: Seq16,
        now: time::Instant,
        timeout: time::Duration,
    ) -> Result<(), RetransmitError> {
        // Cancel the rtt stopwatch
        let old_stopwatch = self.rtt_stopwatches.remove(&seq);
        if old_stopwatch.is_none() {
            return Err(RetransmitError::SequenceNumberNotFound);
        }

        // Start a new rtt stopwatch
        self.rtt_stopwatches.insert(
            seq,
            RttStopwatch2 {
                stopwatch: RttStopwatch::new(now, timeout),
                invalidate_rtt: true,
            },
        );

        Ok(())
    }

    pub fn rtt_stopwatch(&self, seq: Seq16) -> Option<&RttStopwatch> {
        self.rtt_stopwatches
            .get(&seq)
            .map(|stopwatch| &stopwatch.stopwatch)
    }

    pub fn collect_timeout_sequences(&self, now: time::Instant) -> Vec<Seq16> {
        // Collect all timed out sequences
        let mut sequences = Vec::new();
        for (seq, rtt_stopwatch) in &self.rtt_stopwatches {
            if rtt_stopwatch.stopwatch.has_timed_out(now) {
                sequences.push(*seq);
            }
        }

        sequences
    }

    pub fn send(&mut self, now: time::Instant, timeout: time::Duration) -> Option<Seq16> {
        let Some(seq) = self.send_queue.send() else {
            return None;
        };
        self.rtt_stopwatches.insert(
            seq,
            RttStopwatch2 {
                stopwatch: RttStopwatch::new(now, timeout),
                invalidate_rtt: false,
            },
        );
        Some(seq)
    }

    pub fn ack(&mut self, seq: Seq16, now: time::Instant) -> Option<time::Duration> {
        self.send_queue.ack(seq);
        let Some(rtt_stopwatch) = self.rtt_stopwatches.remove(&seq) else {
            return None;
        };
        match rtt_stopwatch.invalidate_rtt {
            true => None,
            false => Some(rtt_stopwatch.stopwatch.into_rtt(now)),
        }
    }
}

struct RttStopwatch2 {
    stopwatch: RttStopwatch,
    invalidate_rtt: bool,
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
        let mut queue = TimedSendQueue::new(10);
        let now = time::Instant::now();
        let timeout = time::Duration::from_millis(100);
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout),
            Err(RetransmitError::SequenceNumberNotFound)
        );
        assert_eq!(queue.send(now, timeout), Some(Seq16::new(0)));
        assert_eq!(queue.retransmit(Seq16::new(0), now, timeout), Ok(()));
        assert_eq!(
            queue.retransmit(Seq16::new(1), now, timeout),
            Err(RetransmitError::SequenceNumberNotFound)
        );
        let now = now + timeout;
        assert_eq!(queue.collect_timeout_sequences(now), vec![Seq16::new(0)]);
        assert_eq!(queue.ack(Seq16::new(0), now), None);
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout),
            Err(RetransmitError::SequenceNumberNotFound)
        );
    }

    #[test]
    fn rtt() {
        let mut queue = TimedSendQueue::new(10);
        let now = time::Instant::now();
        let timeout = time::Duration::from_millis(100);
        assert_eq!(queue.send(now, timeout), Some(Seq16::new(0)));
        let rtt = time::Duration::from_millis(50);
        let now = now + rtt;
        assert_eq!(queue.ack(Seq16::new(0), now), Some(rtt));
    }
}
