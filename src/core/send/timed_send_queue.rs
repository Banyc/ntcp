use std::{collections::BTreeMap, time};

use seq::Seq16;

use super::{RttStopwatch, SendQueue};

pub struct TimedSendQueue<K> {
    rtt_stopwatches: BTreeMap<Seq16, KeyedRttStopwatch<K>>,
    /// Packets that have been sent but not yet acknowledged
    send_queue: SendQueue,
}

impl<K> TimedSendQueue<K>
where
    K: PartialEq,
{
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
        key: K,
    ) -> Result<(), RetransmitError> {
        // Cancel the rtt stopwatch
        let old_stopwatch = self.rtt_stopwatches.remove(&seq);
        if old_stopwatch.is_none() {
            return Err(RetransmitError::SequenceNumberNotFound);
        }

        // Start a new rtt stopwatch
        self.rtt_stopwatches.insert(
            seq,
            KeyedRttStopwatch {
                stopwatch: RttStopwatch::new(now, timeout),
                key,
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

    pub fn send(&mut self, now: time::Instant, timeout: time::Duration, key: K) -> Option<Seq16> {
        let Some(seq) = self.send_queue.send() else {
            return None;
        };
        self.rtt_stopwatches.insert(
            seq,
            KeyedRttStopwatch {
                stopwatch: RttStopwatch::new(now, timeout),
                key,
            },
        );
        Some(seq)
    }

    pub fn ack(&mut self, seq: Seq16, now: time::Instant, key: K) -> Option<time::Duration> {
        self.send_queue.ack(seq);
        let Some(rtt_stopwatch) = self.rtt_stopwatches.remove(&seq) else {
            return None;
        };
        match rtt_stopwatch.key == key {
            true => Some(rtt_stopwatch.stopwatch.into_rtt(now)),
            false => None,
        }
    }
}

struct KeyedRttStopwatch<K> {
    stopwatch: RttStopwatch,
    key: K,
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
        let key_0 = 0;
        let key_1 = 1;
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout, key_1),
            Err(RetransmitError::SequenceNumberNotFound)
        );
        assert_eq!(queue.send(now, timeout, key_0), Some(Seq16::new(0)));
        assert_eq!(queue.retransmit(Seq16::new(0), now, timeout, key_1), Ok(()));
        assert_eq!(
            queue.retransmit(Seq16::new(1), now, timeout, key_1),
            Err(RetransmitError::SequenceNumberNotFound)
        );
        let now = now + timeout;
        assert_eq!(queue.collect_timeout_sequences(now), vec![Seq16::new(0)]);
        assert_eq!(queue.ack(Seq16::new(0), now, key_0), None);
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout, key_1),
            Err(RetransmitError::SequenceNumberNotFound)
        );
    }

    #[test]
    fn rtt() {
        let mut queue = TimedSendQueue::new(10);
        let now = time::Instant::now();
        let timeout = time::Duration::from_millis(100);
        let key_0 = 0;
        assert_eq!(queue.send(now, timeout, key_0), Some(Seq16::new(0)));
        let rtt = time::Duration::from_millis(50);
        let now = now + rtt;
        assert_eq!(queue.ack(Seq16::new(0), now, key_0), Some(rtt));
    }
}
