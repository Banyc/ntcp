use std::{collections::HashMap, time};

use seq::Seq16;

use super::SendQueue;

pub struct RetransmitQueue {
    /// The time at which each packet was sent
    tx_time: HashMap<Seq16, time::Instant>,
    /// Packets that have been sent but not yet acknowledged
    send_queue: SendQueue,
}

impl RetransmitQueue {
    pub fn new(capacity: usize) -> Self {
        Self {
            tx_time: HashMap::new(),
            send_queue: SendQueue::new(capacity),
        }
    }

    pub fn retransmit(
        &self,
        seq: Seq16,
        now: time::Instant,
        timeout: time::Duration,
    ) -> Result<RetransmitResult, RetransmitError> {
        let Some(tx_time) = self.tx_time.get(&seq) else {
            return Err(RetransmitError::SequenceNumberNotFound);
        };
        if now - *tx_time >= timeout {
            return Ok(RetransmitResult::Timeout);
        }
        Ok(RetransmitResult::Waiting)
    }

    pub fn send(&mut self, now: time::Instant) -> Option<Seq16> {
        let Some(seq) = self.send_queue.send() else {
            return None;
        };
        self.tx_time.insert(seq, now);
        Some(seq)
    }

    pub fn ack(&mut self, seq: Seq16) {
        self.send_queue.ack(seq);
        self.tx_time.remove(&seq);
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum RetransmitResult {
    Waiting,
    Timeout,
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
        assert_eq!(queue.send(now), Some(Seq16::new(0)));
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout),
            Ok(RetransmitResult::Waiting)
        );
        assert_eq!(
            queue.retransmit(Seq16::new(1), now, timeout),
            Err(RetransmitError::SequenceNumberNotFound)
        );
        let now = now + timeout;
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout),
            Ok(RetransmitResult::Timeout)
        );
        queue.ack(Seq16::new(0));
        assert_eq!(
            queue.retransmit(Seq16::new(0), now, timeout),
            Err(RetransmitError::SequenceNumberNotFound)
        );
    }
}
