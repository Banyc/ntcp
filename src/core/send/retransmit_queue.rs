use std::time;

use seq::Seq16;

use super::TimedSendQueue;

pub struct RetransmitQueue {
    timed_send_queue: TimedSendQueue,
}

impl RetransmitQueue {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            timed_send_queue: TimedSendQueue::new(capacity),
        }
    }

    #[must_use]
    pub fn retransmit(
        &mut self,
        seq: Seq16,
        now: time::Instant,
    ) -> Result<RetransmitResult, RetransmitError> {
        let Some(rtt_stopwatch) = self.timed_send_queue.rtt_stopwatch(seq) else {
            return Err(RetransmitError::SequenceNumberNotFound);
        };
        if !rtt_stopwatch.has_timed_out(now) {
            return Ok(RetransmitResult::Wait);
        }

        // Cancel the rtt stopwatch
        // Do not start a new rtt stopwatch here
        self.timed_send_queue.cancel_rtt_stopwatch(seq);

        Ok(RetransmitResult::Retransmit)
    }

    pub fn send(&mut self, now: time::Instant, timeout: time::Duration) -> Option<Seq16> {
        self.timed_send_queue.send(now, timeout)
    }

    pub fn ack(&mut self, seq: Seq16, now: time::Instant) -> Option<time::Duration> {
        self.timed_send_queue.ack(seq, now)
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
            queue.retransmit(Seq16::new(0), now),
            Err(RetransmitError::SequenceNumberNotFound)
        );
        assert_eq!(queue.send(now, timeout), Some(Seq16::new(0)));
        assert_eq!(
            queue.retransmit(Seq16::new(0), now),
            Ok(RetransmitResult::Wait)
        );
        assert_eq!(
            queue.retransmit(Seq16::new(1), now),
            Err(RetransmitError::SequenceNumberNotFound)
        );
        let now = now + timeout;
        assert_eq!(
            queue.retransmit(Seq16::new(0), now),
            Ok(RetransmitResult::Retransmit)
        );
        assert_eq!(queue.ack(Seq16::new(0), now), None);
        assert_eq!(
            queue.retransmit(Seq16::new(0), now),
            Err(RetransmitError::SequenceNumberNotFound)
        );
    }

    #[test]
    fn rtt() {
        let mut queue = RetransmitQueue::new(10);
        let now = time::Instant::now();
        let timeout = time::Duration::from_millis(100);
        assert_eq!(queue.send(now, timeout), Some(Seq16::new(0)));
        let rtt = time::Duration::from_millis(50);
        let now = now + rtt;
        assert_eq!(queue.ack(Seq16::new(0), now), Some(rtt));
    }
}
