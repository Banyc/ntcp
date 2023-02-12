mod decode;

pub use decode::*;
use seq::Seq16;

/// # Format
///
/// ```text
/// ( Frame type, ... )
/// ```
///
/// - Frame type field length: `u8`
pub enum Frame {
    Payload(Payload),
    PayloadAck(PayloadAck),
    Ping(Ping),
    PingAck(PingAck),
    Connect(Connect),
}

/// # Format
///
/// ```text
/// ( 0, Seq, Data size, Data )
/// ```
///
/// - Data size field length: `u16`
pub struct Payload {
    pub seq: Seq16,
    pub data: Vec<u8>,
}

/// # Format
///
/// ```text
/// ( 1, Seq )
/// ```
pub struct PayloadAck {
    pub seq: Seq16,
}

/// # Format
///
/// ```text
/// ( 2, Seq )
/// ```
pub struct Ping {
    pub seq: Seq16,
}

/// # Format
///
/// ```text
/// ( 3, Seq )
/// ```
pub struct PingAck {
    pub seq: Seq16,
}

/// # Format
///
/// ```text
/// ( 4, Connection ID )
/// ```
pub struct Connect {
    pub connection_id: u32,
}
