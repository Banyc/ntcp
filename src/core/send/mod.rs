mod rtt_stopwatch;
mod scheduler;
mod send_queue;
mod sockets;
mod timed_send_queue;

use std::{collections::HashMap, os::fd::RawFd, time};

pub use rtt_stopwatch::*;
pub use scheduler::*;
pub use send_queue::*;
use seq::Seq16;
pub use timed_send_queue::*;

use self::sockets::{Credit, ReassignPayloadError, RetransmitPayloads, Sockets};

pub struct Send {
    sockets: Sockets,
    scheduler: Scheduler,
    payload_queue: TimedSendQueue<RawFd>,

    default_rto: time::Duration,
}

impl Send {
    #[must_use]
    pub fn new(config: SendConfig) -> Self {
        Self {
            sockets: Sockets::new(),
            scheduler: Scheduler::new(Vec::new().into_iter(), config.learning_rate),
            payload_queue: TimedSendQueue::new(config.payload_queue_size),
            default_rto: config.default_rto,
        }
    }

    pub fn add_fd(&mut self, fd: RawFd) {
        self.sockets.add_fd(fd);

        self.update_scheduler();
    }

    /// Ignoring the error causes data loss.
    #[must_use]
    pub fn remove_fd(&mut self, fd: RawFd) -> Result<RetransmitPayloads, ReassignPayloadError> {
        let res = self.sockets.remove_fd(fd);

        self.update_scheduler();

        res
    }

    #[must_use]
    pub fn send(&mut self, now: time::Instant, payload_size: usize) -> Vec<SendFrame> {
        let mut payload_size_left = payload_size;
        let mut pings = Vec::new();
        let mut payloads = Vec::new();
        for (&fd, socket) in self.sockets.sockets() {
            // Calculate payload size with ceiling
            let weight = match self.scheduler.weight(&fd) {
                Some(weight) => weight,
                None => {
                    // Even weight
                    1.0 / self.sockets.sockets().len() as f64
                }
            };
            let payload_size = payload_size as f64 * weight;
            let payload_size = payload_size.ceil() as usize;

            // Make sure not exceed payload size
            let payload_size = usize::min(payload_size, payload_size_left);
            payload_size_left -= payload_size;

            // If no payload to send, then send a ping instead
            if payload_size == 0 {
                pings.push(fd);

                // Else: there is an outstanding ping, so don't send another one
                continue;
            }

            // Get timeout
            let timeout = socket
                .rtt()
                .map(|rtt| rtt * 2)
                .unwrap_or_else(|| self.default_rto);

            // Send payload
            payloads.push((fd, payload_size, timeout));
        }
        assert_eq!(payload_size_left, 0);

        // Collect frames
        let mut frames = Vec::new();

        // Send pings
        for fd in pings {
            if let Some(seq) = self.sockets.send_ping(fd, now) {
                frames.push(SendFrame::Ping(PingSendFrame { fd, seq }));
            }
        }

        // Send payloads
        for (fd, payload_size, timeout) in payloads {
            if let Some(seq) = self.payload_queue.send(now, timeout, fd) {
                self.sockets.send_payload(fd, seq);
                frames.push(SendFrame::Payload(PayloadSendFrame {
                    fd,
                    seq,
                    payload_size,
                }));
            }
        }

        frames
    }

    pub fn ack(&mut self, now: time::Instant, fd: RawFd, seq: Seq16, space: AckSpace) {
        // Ack the payload in `payload_queue`
        let space = match space {
            AckSpace::Payload => {
                let rtt = self.payload_queue.ack(seq, now, fd);
                sockets::AckSpace::Payload { rtt }
            }
            AckSpace::Ping => sockets::AckSpace::Ping { now },
        };

        // Ack the socket-related data
        self.sockets.ack(fd, seq, space);
    }

    /// Ignoring the error does not cause data loss.
    #[must_use]
    pub fn retransmit_rto_payloads(
        &mut self,
        now: time::Instant,
    ) -> Result<RetransmitPayloads, ReassignPayloadError> {
        // Reassign RTO payloads to other credible sockets
        let vec = self.payload_queue.collect_timeout_sequences(now);
        let res = self.sockets.reassign_rto_payloads(&vec);

        // Update scheduler
        self.update_scheduler();

        res
    }

    fn update_scheduler(&mut self) {
        let mut rtts = HashMap::new();
        for (&fd, socket) in self.sockets.sockets() {
            if socket.credit() == Credit::Bad {
                continue;
            }
            if let Some(rtt) = socket.rtt() {
                rtts.insert(fd, rtt.as_secs_f64());
            }
        }
        self.scheduler.update(&rtts);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SendConfig {
    pub payload_queue_size: usize,
    pub default_rto: time::Duration,
    pub learning_rate: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SendFrame {
    Payload(PayloadSendFrame),
    Ping(PingSendFrame),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PayloadSendFrame {
    pub fd: RawFd,
    pub seq: Seq16,
    pub payload_size: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PingSendFrame {
    pub fd: RawFd,
    pub seq: Seq16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AckSpace {
    Payload,
    Ping,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ok() {
        let config = SendConfig {
            payload_queue_size: 100,
            default_rto: time::Duration::from_secs(1),
            learning_rate: 0.1,
        };
        let mut send = Send::new(config);

        let fd1 = 1;
        let fd2 = 2;
        let fd3 = 3;

        send.add_fd(fd1);
        send.add_fd(fd2);
        send.add_fd(fd3);

        let now = time::Instant::now();

        // Send 1 payload
        let frames = send.send(now, 3);
        assert_eq!(frames.len(), 3);

        let mut fd1_count = 0;
        let mut fd2_count = 0;
        let mut fd3_count = 0;
        for frame in &frames {
            match frame {
                SendFrame::Payload(frame) => match frame.fd {
                    fd if fd == fd1 => fd1_count += 1,
                    fd if fd == fd2 => fd2_count += 1,
                    fd if fd == fd3 => fd3_count += 1,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
        assert_eq!(fd1_count, 1);
        assert_eq!(fd2_count, 1);
        assert_eq!(fd3_count, 1);

        let duration = time::Duration::from_secs(1);
        let now = now + duration;

        // Ack 1 payload
        for frame in frames {
            match frame {
                SendFrame::Payload(frame) => {
                    send.ack(now, frame.fd, frame.seq, AckSpace::Payload);
                }
                _ => unreachable!(),
            }
        }

        for fd in &[fd1, fd2, fd3] {
            assert_eq!(send.sockets.sockets()[fd].credit(), Credit::Good);
        }
    }

    #[test]
    fn rto_no_rtt() {
        let config = SendConfig {
            payload_queue_size: 100,
            default_rto: time::Duration::from_secs(1),
            learning_rate: 0.1,
        };
        let mut send = Send::new(config);

        let fd1 = 1;
        let fd2 = 2;
        let fd3 = 3;

        send.add_fd(fd1);
        send.add_fd(fd2);
        send.add_fd(fd3);

        let now = time::Instant::now();

        // Send 1 payload
        let frames = send.send(now, 3);
        assert_eq!(frames.len(), 3);
        let frames = frames
            .into_iter()
            .map(|frame| match frame {
                SendFrame::Payload(frame) => frame,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();

        let duration = config.default_rto;
        let now = now + duration;

        // RTO
        let res = send.retransmit_rto_payloads(now);
        assert_eq!(res, Err(ReassignPayloadError::NoSocketsLeft));

        let ack_seq = frames[0].seq;
        let different_fd = frames[1].fd;

        // Ack 1 payload
        send.ack(now, different_fd, ack_seq, AckSpace::Payload);

        // RTO
        let res = send.retransmit_rto_payloads(now);
        assert_eq!(res, Err(ReassignPayloadError::NoSocketsLeft));
    }

    #[test]
    fn rto_ok() {
        let config = SendConfig {
            payload_queue_size: 100,
            default_rto: time::Duration::from_secs(1),
            learning_rate: 0.1,
        };
        let mut send = Send::new(config);

        let fd1 = 1;
        let fd2 = 2;
        let fd3 = 3;

        send.add_fd(fd1);
        send.add_fd(fd2);
        send.add_fd(fd3);

        let now = time::Instant::now();

        // Send 1 payload
        let frames = send.send(now, 3);
        assert_eq!(frames.len(), 3);
        let frames = frames
            .into_iter()
            .map(|frame| match frame {
                SendFrame::Payload(frame) => frame,
                _ => unreachable!(),
            })
            .collect::<Vec<_>>();

        let duration = config.default_rto;
        let now = now + duration;

        // RTO
        let res = send.retransmit_rto_payloads(now);
        assert_eq!(res, Err(ReassignPayloadError::NoSocketsLeft));

        let ack_fd = frames[0].fd;
        let ack_seq = frames[0].seq;

        // Ack 1 payload
        send.ack(now, ack_fd, ack_seq, AckSpace::Payload);

        // RTO
        let retx = send.retransmit_rto_payloads(now).unwrap();
        assert_eq!(retx.len(), 2);
        for (fd, seq) in retx {
            assert_eq!(fd, ack_fd);
            assert!(seq != ack_seq);
        }

        assert_eq!(send.scheduler.weight(&ack_fd).unwrap(), 1.0);
    }

    #[test]
    fn ping_ok() {
        let config = SendConfig {
            payload_queue_size: 100,
            default_rto: time::Duration::from_secs(1),
            learning_rate: 0.1,
        };
        let mut send = Send::new(config);

        let fd1 = 1;
        let fd2 = 2;
        let fd3 = 3;

        send.add_fd(fd1);
        send.add_fd(fd2);
        send.add_fd(fd3);

        let now = time::Instant::now();

        // Send 1 ping
        let frames = send.send(now, 0);
        assert_eq!(frames.len(), 3);

        let mut fd1_count = 0;
        let mut fd2_count = 0;
        let mut fd3_count = 0;
        for frame in &frames {
            match frame {
                SendFrame::Ping(frame) => match frame.fd {
                    fd if fd == fd1 => fd1_count += 1,
                    fd if fd == fd2 => fd2_count += 1,
                    fd if fd == fd3 => fd3_count += 1,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
        assert_eq!(fd1_count, 1);
        assert_eq!(fd2_count, 1);
        assert_eq!(fd3_count, 1);

        let duration = time::Duration::from_secs(1);
        let now = now + duration;

        // Ack 1 ping
        for frame in frames {
            match frame {
                SendFrame::Ping(frame) => {
                    send.ack(now, frame.fd, frame.seq, AckSpace::Ping);
                }
                _ => unreachable!(),
            }
        }

        for fd in &[fd1, fd2, fd3] {
            assert_eq!(send.sockets.sockets()[fd].credit(), Credit::Good);
        }
    }

    #[test]
    fn empty() {
        let config = SendConfig {
            payload_queue_size: 100,
            default_rto: time::Duration::from_secs(1),
            learning_rate: 0.1,
        };
        let mut send = Send::new(config);

        let fd1 = 1;

        send.add_fd(fd1);
        send.remove_fd(fd1).unwrap();
    }
}
