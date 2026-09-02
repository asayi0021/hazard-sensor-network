use lora_phy::mod_params::{Bandwidth, CodingRate, SpreadingFactor};
use heapless::{String,Vec};
use defmt::{info, warn};
use core::fmt::Write; 

/// Packet parameters
pub const MAX_PACKET_LEN: usize = 255;
pub const MAX_PATH_LEN: usize = 64;
pub const MAX_PAYLOAD_LEN: usize = 184;
// JSON formatting parameter
const MAX_JSON_LEN: usize = 128;

/// Modulation parameters
pub const TX_POWER_DBM: i32 = 22; // Assumes the antenna will have 8dBi gain.
pub const FREQ_HZ: u32 = 915_800_000; // Must be in Hz
pub const BANDWIDTH: Bandwidth = Bandwidth::_250KHz; 
pub const SPREADING_FACTOR: SpreadingFactor = SpreadingFactor::_12;
pub const CODING_RATE: CodingRate = CodingRate::_4_8; // _4_5 to _4_8, may need to reduce CR later to increase efficiency.

// RawCustom handling 
/// Single-byte request as RawCustom payload. Only one kind exists for now: "send me
/// everything." Extend with more variants in further iterations
pub const RAW_CUSTOM_REQUEST_TAG: u8 = 0xA0;
pub const RAW_CUSTOM_RESPONSE_TAG: u8 = 0xA1; 

/// Header - Route type (bits 0-1)
#[derive(Debug, Clone, Copy)]
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

/// Functions for pulling info from route type when binary encoded.
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
#[derive(Debug, Clone, Copy)]
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
impl PayloadType {
    pub fn from_bits(b: u8) -> Option<Self> {
        match (b >> 2) & 0x0F {
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

/// Node paramters - Node ID 
pub type NodeIdHash = u8; // u32 for larger hash sizes?

/// Node ID hash size in bytes
#[derive(Debug, Clone, Copy)]
pub enum HashSize {
    One = 0,
    Two = 1,
    Three = 2,
}

/// Functions to move between binary formats for hash size
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

// UNVERIFIED DOWN 
/// Error types for encoding packets 
#[derive(Debug)]
pub enum EncodeError {
    PathTooLong,
    PayloadTooLong,
}

/// Errors when parsing a received frame back into a Packet.
#[derive(Debug)]
pub enum DecodeError {
    TooShort,
    UnknownRouteType,
    UnknownPayloadType,
    ReservedHashSize,
    PathTooLong,
    PayloadTooLong,
}

/// MeshCore packet object 
pub struct Packet<'a> {
    pub payload_version: u8,
    pub route_type: RouteType,
    pub payload_type: PayloadType,
    pub transport_code: Option<u16>,
    pub path: Vec<NodeIdHash, MAX_PATH_LEN>,
    pub payload: &'a [u8],
}

impl<'a> Packet<'a> {
    /// Encode a fresh outbound packet as an originator, used for data broadcasts 
    // (empty path — grows as repeaters forward it)
    pub fn originate(route_type: RouteType, payload_type: PayloadType, payload: &'a [u8]) -> Result<Self, EncodeError> {
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(EncodeError::PayloadTooLong);
        }
        Ok(Self {
            payload_version: 0,
            route_type,
            payload_type,
            transport_code: None,
            path: Vec::new(),
            payload,
        })
    }

    // Send data to public channel is an extension that could be implemented. 
    // Public channel 16-bit hex key 8b3387e9c5cdea6ac9e5edbaa115cd72
    /// Serialize into `out`, returning how many bytes were written.
    pub fn encode(&self, out: &mut [u8; MAX_PACKET_LEN]) -> Result<usize, EncodeError> {
        if self.path.len() > 63 {
            return Err(EncodeError::PathTooLong);
        }
        if self.payload.len() > MAX_PAYLOAD_LEN {
            return Err(EncodeError::PayloadTooLong);
        }

        let mut i = 0usize;

        // Header: version (bits 6-7) | payload type (bits 2-5) | route type (bits 0-1)
        out[i] = ((self.payload_version & 0b11) << 6)
            | (((self.payload_type as u8) & 0x0F) << 2)
            | ((self.route_type as u8) & 0b11);
        i += 1;

        if self.route_type.has_transport_codes() {
            let code = self.transport_code.unwrap_or(0);
            out[i..i + 2].copy_from_slice(&code.to_le_bytes());
            i += 2;
            out[i..i + 2].copy_from_slice(&0u16.to_le_bytes()); // reserved code 2
            i += 2;
        }

        // Path-length byte: hop count in bits 0-5. We only originate with
        // 1-byte node-id hashes, so the hash-size bits (6-7) stay 0b00.
        out[i] = self.path.len() as u8 & 0b0011_1111;
        i += 1;

        for hash in self.path.iter() {
            out[i] = *hash;
            i += 1;
        }

        out[i..i + self.payload.len()].copy_from_slice(self.payload);
        i += self.payload.len();

        Ok(i)
    }

    /// encode_payload: Encode payload into JSON format using the core-fmt crate. 
    /// Currently encodes to Raw-Custom payload type format but a second iteration could 
    /// make use of the Group-Data payload type, which broadcasts to a channel which 
    /// can be accessed by anyone with a channel key. The complication here is that 
    /// to broadcast to the channel the payload must be MAC/AES encrypted using the 
    /// symmetric channel key encryption. For iteration one this is out of scope. 
    pub fn encode_payload(
        wss_data: &i16,                     // Wind speed sensor  
        wds_data: &u16,                     // Wind direction sensor - NEED TO VERIFT SIGNED OR UNSIGNED
        aqs_data: &(i32, u32, u32, i32),    // 4-tuple of gas sensor values
        sms_data: &i16,                     // Soil moisture sensor 
        tbs_data: &f32,                     // Tipping bucket sensor - floating point currently
    ) -> Result<String<MAX_JSON_LEN>, EncodeError>{
        let mut payload: String<MAX_JSON_LEN> = String::new();
        write!(
            payload, 
            "{{\"wss\":{},\"wds\":{},\"aqs\":[{},{},{},{}],\"sms\":{},\"tbs\":{}}}",
            wss_data, wds_data, aqs_data.0, aqs_data.1, aqs_data.2, aqs_data.3, sms_data, tbs_data
        )
        .map_err(|_| EncodeError::PayloadTooLong)?;
        Ok(payload)
    }

    /// Parse a received over-the-air frame. Transforms raw bytes [u8] into Packet  
    /// Borrows the payload slice from `buf` so this stays allocation-free. - NEED TO VERIFY THIS PROPERTY OF THE FUNCTION 
    pub fn decode(buf: &'a [u8]) -> Result<Self, DecodeError> {
        if buf.is_empty() {
            return Err(DecodeError::TooShort);
        }
        let header = buf[0];
        let payload_version = (header >> 6) & 0b11;
        let payload_type = PayloadType::from_bits(header).ok_or(DecodeError::UnknownPayloadType)?;
        let route_type = RouteType::from_bits(header).ok_or(DecodeError::UnknownRouteType)?;

        let mut i = 1usize;
        let mut transport_code = None;
        if route_type.has_transport_codes() {
            if buf.len() < i + 4 {
                return Err(DecodeError::TooShort);
            }
            transport_code = Some(u16::from_le_bytes([buf[i], buf[i + 1]]));
            i += 4; // code 1 (used) + code 2 (reserved)
        }

        if buf.len() < i + 1 {
            return Err(DecodeError::TooShort);
        }
        let path_len_byte = buf[i];
        i += 1;
        let hash_size = HashSize::from_bits(path_len_byte).ok_or(DecodeError::ReservedHashSize)?;
        let hop_count = (path_len_byte & 0b0011_1111) as usize;
        let path_bytes = hop_count * hash_size.bytes();

        if buf.len() < i + path_bytes {
            return Err(DecodeError::TooShort);
        }
        let mut path: Vec<NodeIdHash, MAX_PATH_LEN> = Vec::new();
        for chunk in buf[i..i + path_bytes].chunks(hash_size.bytes()) {
            path.push(chunk[0]).map_err(|_| DecodeError::PathTooLong)?;
        }
        i += path_bytes;

        let payload = &buf[i..];
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(DecodeError::PayloadTooLong);
        }

        Ok(Self {
            payload_version,
            route_type,
            payload_type,
            transport_code,
            path,
            payload,
        })
    }
}

/// Prepend the sub-format tag and package as raw bytes ready for Packet::originate.
pub fn build_sensor_data_frame(json: &str) -> Result<heapless::Vec<u8, MAX_PAYLOAD_LEN>, EncodeError> {
    let mut buf: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    buf.push(RAW_CUSTOM_RESPONSE_TAG).map_err(|_| EncodeError::PayloadTooLong)?;
    buf.extend_from_slice(json.as_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;
    Ok(buf)
}
// Depricated code - can be used to extend queries to single sensor query
// pub enum RequestKey {
//     WSS = "WSS", // Wind speed sensor
//     WDS = "WDS", // Wind direction sensor
//     AQS = "AQS", // Air quality sensor 
//     SMS = "Soil", // Soil moisture sensor 
//     TBS = "TBS", // Tipping bucket sensor 
// }

 // Depricated code - can be used to extend queries to single sensor query - to be used in handle_received
                    // if let Ok(request) = PayloadType::Request::decode(pkt.payload) {
                    //     match request{
                    //         RequestKey::WSS => {
                    //
                    //         }
                    //         RequestKey::WDS => {
                    //
                    //         }
                    //         RequestKey::AQS => {
                    //
                    //         }
                    //         RequestKey::SMS => {
                    //
                    //         }
                    //         RequestKey::TBS => {
                    //
                    //         }
                    //     }
                    // }


