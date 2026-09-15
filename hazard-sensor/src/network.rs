use lora_phy::mod_params::{Bandwidth, CodingRate, SpreadingFactor};
use heapless::{String,Vec};
use defmt::{info, warn};
use core::fmt::Write; 
use sha2::{Digest, Sha256};
use aes::Aes128;
use aes::cipher::{BlockCipherEncrypt, BlockCipherDecrypt, Array, KeyInit};
use hmac::{Hmac, Mac};
use embassy_time::Instant;
use core::sync::atomic::{AtomicI32, AtomicBool, Ordering};

// Imported objects from main
use crate::{MESHCORE_TX_BUFF, CLOCK_SYNCED, CLOCK_OFFSET};

//----MeshCore and LoRa related constants---------------------------------------

/// Packet parameters
pub const MAX_PACKET_LEN: usize = 255;
pub const MAX_PATH_LEN: usize = 64;
pub const MAX_PAYLOAD_LEN: usize = 184;
// JSON formatting parameter
pub const MAX_JSON_LEN: usize = 128;
// Public channel key
pub const PUBLIC_CHANNEL_KEY: [u8; 16] = [
    0x8b, 0x33, 0x87, 0xe9, 0xc5, 0xcd, 0xea, 0x6a,
    0xc9, 0xe5, 0xed, 0xba, 0xa1, 0x15, 0xcd, 0x72,
];
// HMAC/AES encryption constants/type
const BLOCK_SIZE: usize = 16;
const MAC_LEN: usize = 2;
type HmacSha256 = Hmac<Sha256>;
// Node ID - constant for prototype, derived from Ed25519 key in standard MeshCore implementation
pub const NODE_ID: NodeIdHash = 0x42; // 66 in decimal format

/// Modulation parameters
pub const TX_POWER_DBM: i32 = 22; // Assumes the antenna will have 8dBi gain.
pub const FREQ_HZ: u32 = 915_800_000; // Must be in Hz
pub const BANDWIDTH: Bandwidth = Bandwidth::_250KHz; 
pub const SPREADING_FACTOR: SpreadingFactor = SpreadingFactor::_12;
pub const CODING_RATE: CodingRate = CodingRate::_4_8; // _4_5 to _4_8, may need to reduce CR later to increase efficiency.

// RawCustom handling - currently used as group text tag as well
/// Single-byte request as RawCustom payload. Only one kind exists for now: "send me
/// everything." Extend with more variants in further iterations
pub const RAW_CUSTOM_REQUEST_TAG: u8 = 0xA0;
pub const RAW_CUSTOM_RESPONSE_TAG: u8 = 0xA1; 

//----MeshCore related enums and implemented functions--------------------------

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
// only first byte of Ed2556 key is used

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

/// Errors types for encoding packets and payloads
#[derive(Debug)]
pub enum EncodeError {
    PathTooLong,
    PayloadTooLong,
}

/// Errors when parsing a received frame/payload back into a packet/payload
#[derive(Debug)]
pub enum DecodeError {
    TooShort,
    UnknownRouteType,
    UnknownPayloadType,
    ReservedHashSize,
    PathTooLong,
    PayloadTooLong,
    MacMismatch
}

// Used for matching keywords to process different request types.
// Currently only implemented request type is to send all data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestKey {
    DataAll,    // Data from full sensor suite 
    // Below keys are for extending request functionality to sensor specific requests.
    // Wss, // = "WSS"     // Wind speed sensor
    // Wds, // = "WDS"     // Wind direction sensor
    // Aqs, // = "AQS"     // Air quality sensor 
    // Sms, // = "SMS"     // Soil moisture sensor 
    // Tbs, // = "TBS"     // Tipping bucket sensor 
}

// Match string format of keywords returning RequestKey type.
impl RequestKey {
    const DATA_ALL_KEYWORD: &'static str = "DATA";
    // Below keys are for extending request functionality to sensor specific requests.
    // const WSS_KEYWORD: &'static str = "WSS";
    // const WDS_KEYWORD: &'static str = "WDS";
    // const AQS_KEYWORD: &'static str = "AQS";
    // const SMS_KEYWORD: &'static str = "SMS";
    // const TBS_KEYWORD: &'static str = "TBS";

    /// `plaintext` is the full decrypted GRP_TXT body: timestamp(4) + text.
    /// We search the tail as a substring rather than requiring an exact
    /// match, so this works regardless of whether the app prepends
    /// "name: " or a flags byte — both real possibilities per the spec.
    pub fn parse(plaintext: &[u8]) -> Option<Self> {
        if plaintext.len() <= 4 {
            return None;
        }
        let text = core::str::from_utf8(&plaintext[4..]).ok()?;
        if text.contains(Self::DATA_ALL_KEYWORD) {
            info!("DATA keyword recieved");
            Some(Self::DataAll)
        } else {
            None
        }
    }
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

//----Transceiver level (frame) functions that implement MeshCore---------------

/// Called for periodic sensor-data broadcast via RawCustom, for public channel
/// broadcast see: send_group_text_sensor_data
pub fn send_sensor_broadcast(
    wss_data: &i16,
    wds_data: &u16,
    aqs_data: &(i32, u32, u32, i32),
    sms_data: &i16,
    tbs_data: &f32,
) -> Result<(), EncodeError> {
    let json = Packet::encode_payload(wss_data, wds_data, aqs_data, sms_data, tbs_data)?;
    let response_payload = build_sensor_data_frame(&json)?; // adds the missing tag byte

    let pkt = Packet::originate(RouteType::Flood, PayloadType::RawCustom, &response_payload)?;
    let mut buf = [0u8; MAX_PACKET_LEN];
    let len = pkt.encode(&mut buf)?;
    let mut frame: heapless::Vec<u8, { MAX_PACKET_LEN + 1 }> = heapless::Vec::new();
    frame.extend_from_slice(&buf[..len]).map_err(|_| EncodeError::PayloadTooLong)?;

    if crate::MESHCORE_TX_BUFF.try_send(frame).is_err() {
        warn!("tx queue full, dropping sensor broadcast");
    }
    Ok(())
}

/// Prepend the sub-format tag and package as raw bytes ready for Packet::originate.
pub fn build_sensor_data_frame(json: &str) -> Result<heapless::Vec<u8, MAX_PAYLOAD_LEN>, EncodeError> {
    let mut buf: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    buf.push(RAW_CUSTOM_RESPONSE_TAG).map_err(|_| EncodeError::PayloadTooLong)?;
    buf.extend_from_slice(json.as_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;
    Ok(buf)
}

// Function to send data to group_text (public channel), for broadcast or response
pub fn send_group_text_sensor_data(
    wss_data: &i16,
    wds_data: &u16,
    aqs_data: &(i32, u32, u32, i32),
    sms_data: &i16,
    tbs_data: &f32,
) -> Result<(), EncodeError> {
    let json = Packet::encode_payload(wss_data, wds_data, aqs_data, sms_data, tbs_data)?;

    // "hazard-sensor-<id>: <json>" — chat-message shape so a human with the
    // app open sees a readable reply, and NODE_ID lets multiple nodes'
    // replies to the same broadcast be told apart.
    let mut response_text: heapless::String<{ MAX_JSON_LEN + 32 }> = heapless::String::new();
    write!(response_text, "hazard-sensor-{:#04x}: {}", NODE_ID, json)
        .map_err(|_| EncodeError::PayloadTooLong)?;

        // TESTING ALTERNATE FORMAT THAT INCLUDES A BYTE FOR FLAGS
    // let timestamp: u32 = get_current_timestamp(); 
    // let mut plaintext: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    // plaintext.extend_from_slice(&timestamp.to_le_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;
    // plaintext.extend_from_slice(response_text.as_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;

    let timestamp: u32 = get_current_timestamp();
    let mut plaintext: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    plaintext.extend_from_slice(&timestamp.to_le_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;
    plaintext.push(0x00).map_err(|_| EncodeError::PayloadTooLong)?; // flags/txt_type byte
    plaintext.extend_from_slice(response_text.as_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;

    let mut ciphertext: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    let mac = encrypt_then_mac(&PUBLIC_CHANNEL_KEY, &plaintext, &mut ciphertext)?;

    let mut payload: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    GroupTextEnvelope::encode(channel_hash(&PUBLIC_CHANNEL_KEY), mac, &ciphertext, &mut payload)?;

    // Flood, not Direct — GRP_TXT is a broadcast channel message, not a
    // point-to-point reply, so there's no reverse path to send it down.
    // Text message requests would follow same structure but differ here,
    // needing direct RouteType.
    let pkt = Packet::originate(RouteType::Flood, PayloadType::GroupText, &payload)?;

    let mut buf = [0u8; MAX_PACKET_LEN];
    let len = pkt.encode(&mut buf)?;
    let mut frame: heapless::Vec<u8, { MAX_PACKET_LEN + 1 }> = heapless::Vec::new();
    frame.extend_from_slice(&buf[..len]).map_err(|_| EncodeError::PayloadTooLong)?;

    if crate::MESHCORE_TX_BUFF.try_send(frame).is_err() {
        warn!("tx queue full, dropping GRP_TXT sensor data");
        // return Err(EncodeError::PayloadTooLong);
    }
    Ok(())
}

/// Current timestamp for use in outgoing packet plaintexts.
pub fn get_current_timestamp() -> u32 {
    let elapsed_secs = Instant::now().as_secs() as i32;
    let offset = CLOCK_OFFSET.load(Ordering::Relaxed);
    (elapsed_secs + offset) as u32
}

/// Adjust the clock offset using a timestamp extracted from a received
/// packet (e.g. a request from the phone app, which has real wall-clock
/// time). Only ever moves the clock forward, never backward — a stale or
/// out-of-order packet shouldn't be able to rewind us, since MeshCore
/// timestamps are meant to increase monotonically for dedup/freshness.
pub fn sync_clock_from_received(received_timestamp: u32) {
    let elapsed_secs = Instant::now().as_secs() as i32;
    let candidate_offset = received_timestamp as i32 - elapsed_secs;
    let current_offset = CLOCK_OFFSET.load(Ordering::Relaxed);
    if candidate_offset > current_offset {
        CLOCK_OFFSET.store(candidate_offset, Ordering::Relaxed);
        CLOCK_SYNCED.store(true, Ordering::Relaxed);
        info!("clock synced from received packet, new offset = {}", candidate_offset);
    }
}

//----All HMAC/AES payload encrypting down--------------------------------------

/// GRP_TXT / GRP_DATA payload structure: channel_hash(1) || MAC(2) || ciphertext
pub struct GroupTextEnvelope<'a> {
    pub channel_hash: u8,
    pub mac: [u8; 2],
    pub ciphertext: &'a [u8],
}

/// Decoding and encoding of GroupTextEnvelope object to and form bytes
impl<'a> GroupTextEnvelope<'a> {
    pub fn decode(buf: &'a [u8]) -> Result<Self, DecodeError> {
        if buf.len() < 3 {
            return Err(DecodeError::TooShort);
        }
        Ok(Self {
            channel_hash: buf[0],
            mac: [buf[1], buf[2]],
            ciphertext: &buf[3..],
        })
    }

    pub fn encode(
        channel_hash: u8,
        mac: [u8; 2],
        ciphertext: &[u8],
        out: &mut heapless::Vec<u8, MAX_PAYLOAD_LEN>,
    ) -> Result<(), EncodeError> {
        out.clear();
        out.push(channel_hash).map_err(|_| EncodeError::PayloadTooLong)?;
        out.extend_from_slice(&mac).map_err(|_| EncodeError::PayloadTooLong)?;
        out.extend_from_slice(ciphertext).map_err(|_| EncodeError::PayloadTooLong)?;
        Ok(())
    }
}

/// First byte of SHA-256(channel key) — how MeshCore identifies which
/// channel a GRP_TXT/GRP_DATA packet belongs to, per payloads.md.
pub fn channel_hash(key: &[u8]) -> u8 {
    Sha256::digest(key)[0]
}

fn aes128_ecb_encrypt_zero_padded(
    key: &[u8; BLOCK_SIZE],
    plaintext: &[u8],
    out: &mut heapless::Vec<u8, MAX_PAYLOAD_LEN>,
) -> Result<(), EncodeError> {
    out.clear();
    let key_arr = Array::from(*key); // [u8; 16] -> Array<u8, U16>, infallible
    let cipher = Aes128::new(&key_arr);
    let mut i = 0;
    loop {
        let mut block = Array::from([0u8; BLOCK_SIZE]);
        let end = (i + BLOCK_SIZE).min(plaintext.len());
        block[..end - i].copy_from_slice(&plaintext[i..end]);
        cipher.encrypt_block(&mut block);
        out.extend_from_slice(&block).map_err(|_| EncodeError::PayloadTooLong)?;
        i += BLOCK_SIZE;
        if i >= plaintext.len().max(1) { break; }
    }
    Ok(())
}

fn aes128_ecb_decrypt(
    key: &[u8; BLOCK_SIZE],
    ciphertext: &[u8],
    out: &mut heapless::Vec<u8, MAX_PAYLOAD_LEN>,
) -> Result<(), DecodeError> {
    if ciphertext.is_empty() || ciphertext.len() % BLOCK_SIZE != 0 {
        return Err(DecodeError::TooShort);
    }
    out.clear();
    let key_arr = Array::from(*key);
    let cipher = Aes128::new(&key_arr);
    for chunk in ciphertext.chunks(BLOCK_SIZE) {
        let fixed: [u8; BLOCK_SIZE] = chunk.try_into().expect("chunk is exactly BLOCK_SIZE by construction");
        let mut block = Array::from(fixed);
        cipher.decrypt_block(&mut block);
        out.extend_from_slice(&block).map_err(|_| DecodeError::PayloadTooLong)?;
    }
    Ok(())
}

fn compute_mac(key: &[u8], ciphertext: &[u8]) -> [u8; MAC_LEN] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(ciphertext);
    let full = mac.finalize().into_bytes();
    let mut truncated = [0u8; MAC_LEN];
    truncated.copy_from_slice(&full[..MAC_LEN]);
    truncated
}

pub fn encrypt_then_mac(
    key: &[u8; BLOCK_SIZE],
    plaintext: &[u8],
    ciphertext_out: &mut heapless::Vec<u8, MAX_PAYLOAD_LEN>,
) -> Result<[u8; MAC_LEN], EncodeError> {
    aes128_ecb_encrypt_zero_padded(key, plaintext, ciphertext_out)?;
    Ok(compute_mac(key, ciphertext_out))
}

pub fn mac_then_decrypt(
    key: &[u8; BLOCK_SIZE],
    expected_mac: [u8; MAC_LEN],
    ciphertext: &[u8],
    plaintext_out: &mut heapless::Vec<u8, MAX_PAYLOAD_LEN>,
) -> Result<(), DecodeError> {
    if compute_mac(key, ciphertext) != expected_mac {
        return Err(DecodeError::MacMismatch);
    }
    aes128_ecb_decrypt(key, ciphertext, plaintext_out)
}

