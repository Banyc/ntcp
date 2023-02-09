use std::collections::BTreeSet;

use seq::Seq16;

pub struct ReceiveQueue {
    /// The queue of received packets
    queue: BTreeSet<Seq16>,
    /// The maximum number of packets that can be stored in the queue
    capacity: usize,
    /// The first sequence of the receive window
    shadow_first: Seq16,
}

impl ReceiveQueue {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: BTreeSet::new(),
            capacity,
            shadow_first: Seq16::new(0),
        }
    }

    #[must_use]
    pub fn receive(&mut self, seq: Seq16) -> ReceiveResult {
        // Reject out of bounds packets
        if seq < self.shadow_first {
            return ReceiveResult::Reject;
        }
        if Seq16::dist(&self.shadow_first, &seq) as usize >= self.capacity {
            return ReceiveResult::Reject;
        }

        // Insert the new packet
        self.queue.insert(seq);

        return ReceiveResult::Accept;
    }

    #[must_use]
    pub fn pop(&mut self) -> Option<Seq16> {
        let first = self.queue.iter().next().copied();
        if let Some(first) = first {
            if first != self.shadow_first {
                return None;
            }
            self.queue.remove(&first);
            self.shadow_first = first.add(1);
        }
        first
    }

    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum ReceiveResult {
    Reject,
    Accept,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok() {
        let mut queue = ReceiveQueue::new(2);
        assert_eq!(queue.receive(Seq16::new(1)), ReceiveResult::Accept);
        assert_eq!(queue.receive(Seq16::new(2)), ReceiveResult::Reject);
        let first = queue.pop();
        assert_eq!(first, None);
        assert_eq!(queue.receive(Seq16::new(0)), ReceiveResult::Accept);
        let first = queue.pop();
        assert_eq!(first, Some(Seq16::new(0)));
        assert_eq!(queue.receive(Seq16::new(0)), ReceiveResult::Reject);
        let first = queue.pop();
        assert_eq!(first, Some(Seq16::new(1)));
        let first = queue.pop();
        assert_eq!(first, None);
    }

    #[test]
    fn reset_capacity() {
        let mut queue = ReceiveQueue::new(2);
        assert_eq!(queue.receive(Seq16::new(0)), ReceiveResult::Accept);
        assert_eq!(queue.receive(Seq16::new(1)), ReceiveResult::Accept);
        queue.set_capacity(1);
        assert_eq!(queue.receive(Seq16::new(1)), ReceiveResult::Reject);
        let first = queue.pop();
        assert_eq!(first, Some(Seq16::new(0)));
        let first = queue.pop();
        assert_eq!(first, Some(Seq16::new(1)));
        let first = queue.pop();
        assert_eq!(first, None);
    }
}
