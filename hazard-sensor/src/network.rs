use lora_phy::mod_params::{Bandwidth, CodingRate, SpreadingFactor};

/// Packet parameters
pub const MAX_PACKET_LEN: usize = 255;
pub const MAX_PATH_LEN: usize = 64;
pub const MAX_PAYLOAD_LEN: usize = 184;

/// Modulation parameters
pub const TX_POWER_DBM: i32 = 22; // Assumes the antenna will have 8dBi gain.
pub const FREQ_HZ: u32 = 915_800_000; // Must be in Hz
pub const BANDWIDTH: Bandwidth = Bandwidth::_250KHz; 
pub const SPREADING_FACTOR: SpreadingFactor = SpreadingFactor::_12;
pub const CODING_RATE: CodingRate = CodingRate::_4_8; // _4_5 to _4_8, may need to reduce CR later to increase efficiency.

/// Header - Route type (bits 0-1)
pub enum RouteType {
    // Flood routing + Transport codes
    TransportFlood = 0b00,
    // Flood routing
    Flood = 0b01,
    // Direct routing 
    Direct = 0b10,
    // Direct routing + Transport codes
    TransportDirect = 0b11
}

// Functions for pulling info from route type when binary encoded.
// need to bitshift to match on correct bits
impl RouteType {
    pub fn from_bits(b: u8) -> Option<Self> {
        match b & 0b11 {
            0b00 => Some(Self::TransportFlood),
            0b01 => Some(Self::Flood),
            0b10 => Some(Self::Direct),
            0b11 => Some(Self::TransportDirect),
            _ => None
        }
    }
    pub fn has_transport_codes(self) -> bool {
        matches!(self, Self::TransportFlood | Self::TransportDirect)
    }
}

/// Header - Payload type (bits 2-5)
pub enum PayloadType {
    Request = 0x00,
    Response = 0x01,
    TextMessage = 0x02,
    Ack = 0x03,
    Advert = 0x04,
    GroupText = 0x05,
    GroupData = 0x06,
    AnonRequest = 0x07,
    Path = 0x08,
    Trace = 0x09,
    Multipart = 0x0A,
    Control = 0x0B,
    RawCustom = 0x0F,
}

/// Header - Payload type (bits 2-5) 
// need to bitshift to match on correct bits
impl PayloadType {
    pub fn from_bits(b: u8) -> Option<Self> {
        match b & 0x0F {
            0x00 => Some(Self::Request),
            0x01 => Some(Self::Response),
            0x02 => Some(Self::TextMessage),
            0x03 => Some(Self::Ack),
            0x04 => Some(Self::Advert),
            0x05 => Some(Self::GroupText),
            0x06 => Some(Self::GroupData),
            0x07 => Some(Self::AnonRequest),
            0x08 => Some(Self::Path),
            0x09 => Some(Self::Trace),
            0x0A => Some(Self::Multipart),
            0x0B => Some(Self::Control),
            0x0F => Some(Self::RawCustom),
            _ => None,
        }
    }
}

pub type NodeIdHash = u8; // u32 for larger hash sizes?

pub enum HashSize {
    One = 0,
    Two = 1,
    Three = 2,
}

impl HashSize {
    pub fn bytes(self) -> usize {
        match self {
            HashSize::One => 1,
            HashSize::Two => 2,
            HashSize::Three => 3,
        }
    }

    pub fn from_bits(b: u8) -> Option<Self> {
        match (b >> 6) & 0b11 {
            0 => Some(HashSize::One),
            1 => Some(HashSize::Two),
            2 => Some(HashSize::Three),
            _ => None,
        }
    }
}
