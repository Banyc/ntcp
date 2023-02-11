use std::{
    collections::{BTreeSet, HashMap},
    os::fd::RawFd,
    time,
};

use rep::*;
use seq::Seq16;

use super::TimedSendQueue;

#[derive(CheckIndieFields)]
pub struct Sockets {
    /// Payload-to-socket mappings
    payload_fds: HashMap<Seq16, RawFd>,

    sockets: HashMap<RawFd, Socket>,
}

impl CheckFields for Sockets {
    fn check_fields(&self, e: &mut RepErrors) {
        // Check payload-to-socket-to-payload consistency
        for (seq, fd) in self.payload_fds.iter() {
            let Some(socket) = self.sockets.get(fd) else {
                e.add(format!(
                    "Payload {:?} is assigned to socket {}, but socket {} does not exist",
                    seq, fd, fd
                ));
                continue;
            };
            let true = socket.payloads.contains(seq) else {
                e.add(format!(
                    "Payload {:?} is assigned to socket {}, but socket {} does not have it",
                    seq, fd, fd
                ));
                continue;
            };
        }

        // Check socket-to-payload-to-socket consistency
        for (fd, socket) in self.sockets.iter() {
            for seq in socket.payloads.iter() {
                let Some(payload_fd) = self.payload_fds.get(seq) else {
                    e.add(format!(
                        "Socket {} has payload {:?}, but payload {:?} is not assigned to any socket",
                        fd, seq, seq
                    ));
                    continue;
                };
                let true = payload_fd == fd else {
                    e.add(format!(
                        "Socket {} has payload {:?}, but payload {:?} is assigned to socket {}",
                        fd, seq, seq, payload_fd
                    ));
                    continue;
                };
            }
        }
    }
}

impl CheckRep for Sockets {}

#[check_rep]
impl Sockets {
    #[must_use]
    pub fn new() -> Self {
        Self {
            payload_fds: HashMap::new(),
            sockets: HashMap::new(),
        }
    }

    pub fn add_fd(&mut self, fd: RawFd) {
        self.sockets.insert(fd, Socket::new());
    }

    #[must_use]
    pub fn remove_fd(&mut self, fd: RawFd) -> Result<RetransmitPayloads, ReassignPayloadError> {
        let Some(socket) = self.sockets.remove(&fd) else {
            // Socket was already removed
            return Ok(Vec::new());
        };

        // Remove relative payload-to-socket mappings
        for seq in socket.payloads.iter() {
            self.payload_fds.remove(seq);
        }

        if socket.payloads.is_empty() {
            // No payloads to reassign
            return Ok(Vec::new());
        };

        if self.sockets.len() == 0 {
            // No sockets left to reassign payloads to
            return Err(ReassignPayloadError::NoSocketsLeft {
                payloads: socket.payloads.into_iter().collect(),
            });
        };

        // The remaining sockets will be assigned the payloads of the removed socket
        let applicable_sockets = self.sockets.keys().copied().collect();

        // Round-robin assign payloads to other sockets
        self.round_robin_reassign_payloads(socket.payloads.into_iter(), applicable_sockets)
    }

    #[must_use]
    pub fn send_ping(&mut self, fd: RawFd, now: time::Instant) -> Option<Seq16> {
        let Some(socket) = self.sockets.get_mut(&fd) else {
            // Socket was already removed
            return None;
        };
        socket
            .ping_queue
            .send(now, time::Duration::from_secs(0), fd)
    }

    #[must_use]
    pub fn sockets(&self) -> &HashMap<RawFd, Socket> {
        &self.sockets
    }

    pub fn send_payload(&mut self, fd: RawFd, seq: Seq16) {
        self.reassign_payload_seq(fd, seq);
    }

    pub fn ack(&mut self, receiving_fd: RawFd, seq: Seq16, space: AckSpace) {
        // Summarize RTT
        let (socket, rtt) = match space {
            AckSpace::Payload { rtt } => {
                let Some(assigned_fd) = self.remove_payload_seq(seq) else {
                    // Payload was already acked
                    return;
                };
                if assigned_fd != receiving_fd {
                    // Payload was retransmitted on a different socket (`assigned_fd`) than the ACK-receiving socket (`receiving_fd`)
                    // Ack the payload on the `assigned_fd` socket
                };
                let Some(socket) = self.sockets.get_mut(&assigned_fd) else {
                    // Socket was already removed
                    return;
                };

                (socket, rtt)
            }
            AckSpace::Ping { now } => {
                let Some(socket) = self.sockets.get_mut(&receiving_fd) else {
                    return;
                };
                let rtt = socket.ping_queue.ack(seq, now, receiving_fd);
                (socket, rtt)
            }
        };

        // Update socket RTT and credit
        if let Some(rtt) = rtt {
            socket.rtt = Some(rtt);
            socket.credit = Credit::Good;
        }
    }

    /// Prevent the socket from being assigned with RTO payloads
    fn discredit(&mut self, seq: Seq16) {
        if let Some(socket) = self.socket_mut(seq) {
            socket.credit = Credit::Bad;
        }
    }

    #[must_use]
    pub fn reassign_rto_payloads(
        &mut self,
        rto_payloads: &[Seq16],
    ) -> Result<RetransmitPayloads, ReassignPayloadError> {
        // Discredit sockets that have caused RTOs
        for seq in rto_payloads {
            self.discredit(*seq);
        }

        let applicable_sockets = self
            .sockets
            .iter()
            .filter_map(|(fd, socket)| {
                if let Credit::Good = socket.credit {
                    Some(*fd)
                } else {
                    None
                }
            })
            .collect();

        self.round_robin_reassign_payloads(rto_payloads.iter().map(|seq| *seq), applicable_sockets)
    }

    #[must_use]
    fn round_robin_reassign_payloads(
        &mut self,
        payloads: impl IntoIterator<Item = Seq16>,
        applicable_sockets: Vec<RawFd>,
    ) -> Result<RetransmitPayloads, ReassignPayloadError> {
        if applicable_sockets.len() == 0 {
            return Err(ReassignPayloadError::NoSocketsLeft {
                payloads: payloads.into_iter().collect(),
            });
        };

        let mut assigned_payloads = Vec::new();

        // Round-robin assign payloads to other sockets
        let mut round_robin = applicable_sockets.iter().cycle();
        for seq in payloads {
            let Some(assignee) = round_robin.next() else {
                unreachable!();
            };
            assigned_payloads.push((*assignee, seq));

            // Reassign the payload to the new socket
            self.reassign_payload_seq(*assignee, seq)
        }

        Ok(assigned_payloads)
    }

    fn reassign_payload_seq(&mut self, assignee: RawFd, seq: Seq16) {
        // Remove the payload from the old socket
        self.remove_payload_seq(seq);

        // Assign the payload to the new socket
        self.payload_fds.insert(seq, assignee);
        if let Some(socket) = self.socket_mut(seq) {
            socket.payloads.insert(seq);
        }
    }

    fn remove_payload_seq(&mut self, seq: Seq16) -> Option<RawFd> {
        // Remove fd -> seq mapping
        if let Some(socket) = self.socket_mut(seq) {
            socket.payloads.remove(&seq);
        }

        // Remove seq -> fd mapping
        let fd = self.payload_fds.remove(&seq);

        fd
    }

    /// Return `None` if either:
    ///
    /// - Payload was already acked
    /// - Socket was already removed
    fn socket_mut(&mut self, seq: Seq16) -> Option<&mut Socket> {
        let Some(fd) = self.payload_fds.get(&seq) else {
            // Payload was already acked
            return None;
        };
        match self.sockets.get_mut(&fd) {
            Some(socket) => Some(socket),
            None => {
                // Socket was already removed
                self.payload_fds.remove(&seq);
                None
            }
        }
    }
}

pub struct Socket {
    ping_queue: TimedSendQueue<RawFd>,
    rtt: Option<time::Duration>,
    payloads: BTreeSet<Seq16>,
    credit: Credit,
}

impl Socket {
    #[must_use]
    pub fn new() -> Self {
        Self {
            ping_queue: TimedSendQueue::new(1),
            rtt: None,
            payloads: BTreeSet::new(),
            credit: Credit::Bad,
        }
    }

    pub fn rtt(&self) -> Option<time::Duration> {
        self.rtt
    }

    pub fn credit(&self) -> Credit {
        self.credit
    }
}

/// Good -> bad: RTO exceeded
/// Bad -> good: New RTT sample updated
#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum Credit {
    Good,
    Bad,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Hash)]
pub enum AckSpace {
    Payload { rtt: Option<time::Duration> },
    Ping { now: time::Instant },
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ReassignPayloadError {
    NoSocketsLeft { payloads: BTreeSet<Seq16> },
}

pub type RetransmitPayloads = Vec<(RawFd, Seq16)>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok() {
        let mut sockets = Sockets::new();
        let fd1 = 1;
        let fd2 = 2;
        let fd3 = 3;

        sockets.add_fd(fd1);
        sockets.remove_fd(fd1).unwrap();

        sockets.add_fd(fd1);
        sockets.add_fd(fd2);
        sockets.add_fd(fd3);

        let seq1 = Seq16::new(0);
        let seq2 = Seq16::new(1);

        let now = time::Instant::now();
        sockets.send_payload(fd1, seq1);
        sockets.send_payload(fd2, seq2);
        let seq3 = sockets.send_ping(fd3, now).unwrap();

        assert_eq!(seq3, Seq16::new(0));
        assert!(sockets.send_ping(fd3, now).is_none());

        let duration = time::Duration::from_millis(100);
        let now = now + duration;

        sockets.discredit(seq1);
        sockets.discredit(seq2);
        sockets.discredit(seq3);

        sockets.ack(fd1, seq1, AckSpace::Payload { rtt: None });
        sockets.ack(
            fd2,
            seq2,
            AckSpace::Payload {
                rtt: Some(duration),
            },
        );
        sockets.ack(fd3, seq3, AckSpace::Ping { now });

        assert_eq!(sockets.sockets[&fd1].rtt(), None);
        assert_eq!(sockets.sockets[&fd2].rtt(), Some(duration));
        assert_eq!(sockets.sockets[&fd3].rtt(), Some(duration));

        assert_eq!(sockets.sockets[&fd1].credit, Credit::Bad);
        assert_eq!(sockets.sockets[&fd2].credit, Credit::Good);
        assert_eq!(sockets.sockets[&fd3].credit, Credit::Good);
    }

    #[test]
    fn reassign_on_remove_fd() {
        let mut sockets = Sockets::new();
        let fd1 = 1;
        let fd2 = 2;
        let fd3 = 3;

        sockets.add_fd(fd1);
        sockets.add_fd(fd2);
        sockets.add_fd(fd3);

        let seq1 = Seq16::new(2);
        sockets.send_payload(fd1, seq1);
        let seq1 = Seq16::new(3);
        sockets.send_payload(fd1, seq1);
        let seq1 = Seq16::new(4);
        sockets.send_payload(fd1, seq1);

        let retx = sockets.remove_fd(fd1).unwrap();
        let mut fd2_count = 0;
        let mut fd3_count = 0;
        let mut seqs = Vec::new();
        for (fd, seq) in retx {
            seqs.push(seq);
            if fd == fd2 {
                fd2_count += 1;
            } else if fd == fd3 {
                fd3_count += 1;
            } else {
                unreachable!();
            }
        }
        assert!(fd2_count > 0);
        assert!(fd3_count > 0);
        assert_eq!(seqs.len(), fd2_count + fd3_count);
        seqs.dedup();
        assert_eq!(seqs.len(), fd2_count + fd3_count);
        for seq in seqs {
            assert!(seq == Seq16::new(2) || seq == Seq16::new(3) || seq == Seq16::new(4));
        }
    }

    #[test]
    fn reassign_on_rto() {
        let mut sockets = Sockets::new();
        let fd1 = 1;
        let fd2 = 2;
        let fd3 = 3;

        sockets.add_fd(fd1);
        sockets.add_fd(fd2);
        sockets.add_fd(fd3);

        let seq1_1 = Seq16::new(0);
        let seq1_2 = Seq16::new(1);
        let seq2_1 = Seq16::new(2);

        let now = time::Instant::now();
        sockets.send_payload(fd1, seq1_1);
        sockets.send_payload(fd1, seq1_2);
        sockets.send_payload(fd2, seq2_1);

        let duration = time::Duration::from_millis(100);
        let _now = now + duration;

        sockets.ack(
            fd2,
            seq2_1,
            AckSpace::Payload {
                rtt: Some(duration),
            },
        );

        assert_eq!(sockets.reassign_rto_payloads(&[]).unwrap().len(), 0);

        let retx_seqs = vec![seq1_1, seq1_2];
        let retx = sockets.reassign_rto_payloads(&retx_seqs).unwrap();

        for (fd, seq) in retx {
            if fd != fd2 {
                unreachable!();
            }
            assert!(seq == seq1_1 || seq == seq1_2);
        }
    }
}
