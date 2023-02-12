use std::io::BufReader;
use std::io::Read;

use byteorder::BigEndian;
use byteorder::ReadBytesExt;
use seq::Seq16;
use thiserror::Error;

use super::Connect;
use super::Frame;
use super::Payload;
use super::PayloadAck;
use super::Ping;
use super::PingAck;

impl TryFrom<&mut BufReader<&[u8]>> for Frame {
    type Error = DecodeError;

    fn try_from(value: &mut BufReader<&[u8]>) -> Result<Self, Self::Error> {
        let Ok(frame_type) = value.read_u8() else {
            return Err(DecodeError::InvalidFrameType);
        };
        match frame_type {
            0 => Ok(Frame::Payload(Payload::try_from(value)?)),
            1 => Ok(Frame::PayloadAck(PayloadAck::try_from(value)?)),
            2 => Ok(Frame::Ping(Ping::try_from(value)?)),
            3 => Ok(Frame::PingAck(PingAck::try_from(value)?)),
            4 => Ok(Frame::Connect(Connect::try_from(value)?)),
            _ => Err(DecodeError::InvalidFrameType),
        }
    }
}

impl TryFrom<&mut BufReader<&[u8]>> for Payload {
    type Error = DecodeError;

    fn try_from(value: &mut BufReader<&[u8]>) -> Result<Self, Self::Error> {
        let seq = parse_seq16(value, DecodeError::InvalidPayload)?;
        let Ok(data_size) = value.read_u16::<BigEndian>() else {
            return Err(DecodeError::InvalidPayload);
        };
        let mut data = vec![0; data_size as usize];
        let Ok(()) = value.read_exact(&mut data) else {
            return Err(DecodeError::InvalidPayload);
        };
        Ok(Payload { seq, data })
    }
}

impl TryFrom<&mut BufReader<&[u8]>> for PayloadAck {
    type Error = DecodeError;

    fn try_from(value: &mut BufReader<&[u8]>) -> Result<Self, Self::Error> {
        Ok(PayloadAck {
            seq: parse_seq16(value, DecodeError::InvalidPayloadAck)?,
        })
    }
}

impl TryFrom<&mut BufReader<&[u8]>> for Ping {
    type Error = DecodeError;

    fn try_from(value: &mut BufReader<&[u8]>) -> Result<Self, Self::Error> {
        Ok(Ping {
            seq: parse_seq16(value, DecodeError::InvalidPing)?,
        })
    }
}

impl TryFrom<&mut BufReader<&[u8]>> for PingAck {
    type Error = DecodeError;

    fn try_from(value: &mut BufReader<&[u8]>) -> Result<Self, Self::Error> {
        Ok(PingAck {
            seq: parse_seq16(value, DecodeError::InvalidPingAck)?,
        })
    }
}

fn parse_seq16(value: &mut BufReader<&[u8]>, err: DecodeError) -> Result<Seq16, DecodeError> {
    let Ok(seq) = value.read_u16::<BigEndian>() else {
        return Err(err);
    };
    Ok(Seq16::new(seq))
}

impl TryFrom<&mut BufReader<&[u8]>> for Connect {
    type Error = DecodeError;

    fn try_from(value: &mut BufReader<&[u8]>) -> Result<Self, Self::Error> {
        let Ok(connection_id) = value.read_u32::<BigEndian>() else {
            return Err(DecodeError::InvalidConnect);
        };
        Ok(Connect { connection_id })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Error)]
pub enum DecodeError {
    #[error("invalid frame type")]
    InvalidFrameType,
    #[error("invalid payload")]
    InvalidPayload,
    #[error("invalid payload ack")]
    InvalidPayloadAck,
    #[error("invalid ping")]
    InvalidPing,
    #[error("invalid ping ack")]
    InvalidPingAck,
    #[error("invalid connect")]
    InvalidConnect,
}
