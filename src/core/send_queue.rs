use std::collections::BTreeSet;

use seq::Seq16;

pub struct SendQueue {
    /// The queue of sending packets
    queue: BTreeSet<Seq16>,
    /// The maximum number of packets that can be stored in the queue
    capacity: usize,
    /// The sequence number of the next new packet
    shadow_end: Seq16,
}

impl SendQueue {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: BTreeSet::new(),
            capacity,
            shadow_end: Seq16::new(0),
        }
    }

    #[must_use]
    pub fn send(&mut self) -> Option<Seq16> {
        // Reject if the queue is full
        if self.queue.len() >= self.capacity {
            return None;
        }

        // Insert the new packet
        let seq = self.shadow_end;
        self.queue.insert(seq);

        // Increment the shadow end
        self.shadow_end = seq.add(1);

        Some(seq)
    }

    pub fn ack(&mut self, seq: Seq16) {
        self.queue.remove(&seq);
    }

    pub fn set_capacity(&mut self, capacity: usize) {
        self.capacity = capacity;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok() {
        let mut queue = SendQueue::new(2);
        assert_eq!(queue.send(), Some(Seq16::new(0)));
        assert_eq!(queue.send(), Some(Seq16::new(1)));
        assert_eq!(queue.send(), None);
        queue.ack(Seq16::new(0));
        assert_eq!(queue.send(), Some(Seq16::new(2)));
        assert_eq!(queue.send(), None);
        queue.ack(Seq16::new(2));
        assert_eq!(queue.send(), Some(Seq16::new(3)));
        assert_eq!(queue.send(), None);
        queue.ack(Seq16::new(1));
        assert_eq!(queue.send(), Some(Seq16::new(4)));
        assert_eq!(queue.send(), None);
    }

    #[test]
    fn reset_capacity() {
        let mut queue = SendQueue::new(2);
        assert_eq!(queue.send(), Some(Seq16::new(0)));
        assert_eq!(queue.send(), Some(Seq16::new(1)));
        assert_eq!(queue.send(), None);
        queue.set_capacity(1);
        assert_eq!(queue.send(), None);
        queue.ack(Seq16::new(0));
        assert_eq!(queue.send(), None);
        queue.ack(Seq16::new(1));
        assert_eq!(queue.send(), Some(Seq16::new(2)));
        assert_eq!(queue.send(), None);
    }
}
