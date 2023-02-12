use super::Connect;
use super::Frame;
use super::Payload;
use super::PayloadAck;
use super::Ping;
use super::PingAck;

impl From<&Frame> for Vec<u8> {
    fn from(frame: &Frame) -> Self {
        let mut buf = Vec::new();
        match frame {
            Frame::Payload(payload) => {
                buf.push(0);
                buf.extend::<Vec<u8>>(payload.into());
            }
            Frame::PayloadAck(payload_ack) => {
                buf.push(1);
                buf.extend::<Vec<u8>>(payload_ack.into());
            }
            Frame::Ping(ping) => {
                buf.push(2);
                buf.extend::<Vec<u8>>(ping.into());
            }
            Frame::PingAck(ping_ack) => {
                buf.push(3);
                buf.extend::<Vec<u8>>(ping_ack.into());
            }
            Frame::Connect(connect) => {
                buf.push(4);
                buf.extend::<Vec<u8>>(connect.into());
            }
        }
        buf
    }
}

impl From<&Payload> for Vec<u8> {
    fn from(payload: &Payload) -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(&payload.seq.value().to_be_bytes());
        buf.extend_from_slice(&(payload.data.len() as u16).to_be_bytes());
        buf.extend_from_slice(&payload.data);
        buf
    }
}

impl From<&PayloadAck> for Vec<u8> {
    fn from(payload_ack: &PayloadAck) -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(&payload_ack.seq.value().to_be_bytes());
        buf
    }
}

impl From<&Ping> for Vec<u8> {
    fn from(ping: &Ping) -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(&ping.seq.value().to_be_bytes());
        buf
    }
}

impl From<&PingAck> for Vec<u8> {
    fn from(ping_ack: &PingAck) -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(&ping_ack.seq.value().to_be_bytes());
        buf
    }
}

impl From<&Connect> for Vec<u8> {
    fn from(connect: &Connect) -> Self {
        let mut buf = Vec::new();
        buf.extend_from_slice(&connect.connection_id.to_be_bytes());
        buf
    }
}
