use lora_phy::mod_params::{Bandwidth, CodingRate, SpreadingFactor};
use heapless::{String,Vec};
use defmt::{info, warn, error};
use core::fmt::Write; 
use sha2::{Digest, Sha256};
use aes::Aes128;
use aes::cipher::{BlockCipherEncrypt, BlockCipherDecrypt, Array, KeyInit};
use hmac::{Hmac, Mac};
use embassy_time::Instant;
use core::sync::atomic::Ordering;

// Imported objects/functions from main and sensors module.
use crate::sensors::{gas_sensor::GasSensor, adc_sensors::AdcSensors, tipping_bucket::RainfallSensor};
use crate::{MESHCORE_TX_BUFF, CLOCK_SYNCED, CLOCK_OFFSET, I2cShared, SX1262}; 


//======================================================================================================================
//----MeshCore/LoRa related constants and parameters--------------------------------------------------------------------
//======================================================================================================================

/// Packet parameters
pub const MAX_PACKET_LEN: usize = 255;
const MAX_PATH_LEN: usize = 64;
const MAX_PAYLOAD_LEN: usize = 184;
// JSON formatting parameter
const MAX_JSON_LEN: usize = 128; // can be upped to MAX_PAYLOAD_LENGTH if need be 
// Public channel key
const PUBLIC_CHANNEL_KEY: [u8; 16] = [
    0x8b, 0x33, 0x87, 0xe9, 0xc5, 0xcd, 0xea, 0x6a,
    0xc9, 0xe5, 0xed, 0xba, 0xa1, 0x15, 0xcd, 0x72,
];
// Node ID - constant for prototype, derived from Ed25519 key in standard 
// MeshCore implementation. The real MeshCore ID is feasible to get but it 
// depends on hosts requirements of ID logging to see if its worth doing.
pub const NODE_ID: NodeIdHash = 0x42; // 66 in decimal format

// HMAC/AES encryption constants/type
const BLOCK_SIZE: usize = 16;
const MAC_LEN: usize = 2;
type HmacSha256 = Hmac<Sha256>;

/// Modulation parameters
pub const TX_POWER_DBM: i32 = 22; // Assumes the antenna will have 8dBi gain.
pub const FREQ_HZ: u32 = 915_800_000; // Must be in Hz
pub const BANDWIDTH: Bandwidth = Bandwidth::_250KHz; 
pub const SPREADING_FACTOR: SpreadingFactor = SpreadingFactor::_12;
pub const CODING_RATE: CodingRate = CodingRate::_4_8; // _4_5 to _4_8, may need to reduce CR later to increase efficiency.

// RawCustom handling - currently used as group text tag as well
/// Single-byte request as RawCustom payload. Only one kind exists for now: "send me
/// everything." Extend with more variants in further iterations
const RAW_CUSTOM_REQUEST_TAG: u8 = 0xA0;
const RAW_CUSTOM_RESPONSE_TAG: u8 = 0xA1; 


//======================================================================================================================
//----MeshCore protcol level firmware-----------------------------------------------------------------------------------
//======================================================================================================================

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

/// Header - Route type (bits 0-1)
#[derive(Debug, Clone, Copy)]
enum RouteType {
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
    fn from_bits(b: u8) -> Option<Self> {
        match b & 0b11 {
            0b00 => Some(Self::TransportFlood),
            0b01 => Some(Self::Flood),
            0b10 => Some(Self::Direct),
            0b11 => Some(Self::TransportDirect),
            _ => None
        }
    }
    fn has_transport_codes(self) -> bool {
        matches!(self, Self::TransportFlood | Self::TransportDirect)
    }
}

/// Header - Payload type (bits 2-5)
#[derive(Debug, Clone, Copy)]
enum PayloadType {
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
    fn from_bits(b: u8) -> Option<Self> {
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
enum HashSize {
    One = 0,
    Two = 1,
    Three = 2,
}

/// Functions to move between binary formats for hash size
impl HashSize {
    fn bytes(self) -> usize {
        match self {
            HashSize::One => 1,
            HashSize::Two => 2,
            HashSize::Three => 3,
        }
    }

    fn from_bits(b: u8) -> Option<Self> {
        match (b >> 6) & 0b11 {
            0 => Some(HashSize::One),
            1 => Some(HashSize::Two),
            2 => Some(HashSize::Three),
            _ => None,
        }
    }
}

// Used for matching keywords to process different request types.
// Currently only implemented request type is to send all data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestKey {
    DataAll,            // Data from full sensor suite 
    // Below keys are for extending request functionality to sensor specific requests.
    Wss, // = "WSS"     // Wind speed sensor
    // Wds, // = "WDS"     // Wind direction sensor
    Aqs, // = "AQS"     // Air quality sensor 
    Sms, // = "SMS"     // Soil moisture sensor 
    Tbs, // = "TBS"     // Tipping bucket sensor 
}

// Match string format of keywords returning RequestKey type.
impl RequestKey {
    const DATA_ALL_KEYWORD: &'static str = "DATA";
    // Below keys are for extending request functionality to sensor specific requests.
    const WSS_KEYWORD: &'static str = "WSS";
    // const WDS_KEYWORD: &'static str = "WDS";
    const AQS_KEYWORD: &'static str = "AQS";
    const SMS_KEYWORD: &'static str = "SMS";
    const TBS_KEYWORD: &'static str = "TBS";

    /// This node's addressing tag, e.g. "#42" for NODE_ID = 0x42. A sender
    /// includes this alongside DATA to target this specific node — e.g.
    /// "DATA #42" — rather than every node on the channel responding to
    /// every DATA broadcast.
    fn node_tag() -> heapless::String<8> {
        let mut s: heapless::String<8> = heapless::String::new();
        let _ = write!(s, "#{:02x}", NODE_ID);
        s
    }

    /// `plaintext` is the full decrypted GRP_TXT body: timestamp(4) + text
    /// (we search the tail as a substring rather than requiring an exact
    /// offset, so this tolerates a flags byte either way).
    fn parse(plaintext: &[u8]) -> Option<Self> {
        if plaintext.len() <= 4 {
            return None;
        }
        let text = core::str::from_utf8(&plaintext[4..]).ok()?;
        let tag = Self::node_tag();
        if !text.contains(tag.as_str()) {
            return None;
        }

        const KEYWORDS: [(&str, RequestKey); 5] = [
            (RequestKey::WSS_KEYWORD, RequestKey::Wss),
            // (RequestKey::WDS_KEYWORD, RequestKey::Wds),
            (RequestKey::AQS_KEYWORD, RequestKey::Aqs),
            (RequestKey::SMS_KEYWORD, RequestKey::Sms),
            (RequestKey::TBS_KEYWORD, RequestKey::Tbs),
            (RequestKey::DATA_ALL_KEYWORD, RequestKey::DataAll),
        ];

        KEYWORDS.iter().find_map(|&(keyword, key)| {
            text.contains(keyword).then_some(key)
        })
    }
}

/// MeshCore packet object 
struct Packet<'a> {
    pub payload_version: u8,
    pub route_type: RouteType,
    pub payload_type: PayloadType,
    pub transport_code: Option<u16>,
    pub path: Vec<NodeIdHash, MAX_PATH_LEN>,
    pub payload: &'a [u8],
}

impl<'a> Packet<'a> {
    /// Encode a fresh outbound packet as an originator, used for data broadcasts 
    fn originate(route_type: RouteType, payload_type: PayloadType, payload: &'a [u8]) -> Result<Self, EncodeError> {
        if payload.len() > MAX_PAYLOAD_LEN {
            return Err(EncodeError::PayloadTooLong);
        }
        Ok(Self {
            payload_version: 0,
            route_type,
            payload_type,
            transport_code: None,
            path: Vec::new(), // (empty path — grows as repeaters forward it)
            payload,
        })
    }

    /// Serialize into `out`, returning how many bytes were written.
    fn encode(&self, out: &mut [u8; MAX_PACKET_LEN]) -> Result<usize, EncodeError> {
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
    fn encode_payload( 
        r: &SensorReadings,
    ) -> Result<String<MAX_JSON_LEN>, EncodeError>{
        let mut payload: String<MAX_JSON_LEN> = String::new();
        write!(
            payload, 
            "{{\"wss\":{},\"aqs_tmp\":{},\"aqs_hum\":{},\"aqs_prs\":{},\"aqs_aqi\":{},\"sms\":{},\"tbs\":{}}}",
            r.wss, r.aqs.0, r.aqs.1, r.aqs.2, r.aqs.3, r.sms, r.tbs
        )
        .map_err(|_| EncodeError::PayloadTooLong)?;
        Ok(payload)
    }

    /// Parse a received over-the-air frame. Transforms raw bytes [u8] into Packet  
    /// Borrows the payload slice from `buf` so this stays allocation-free. - NEED TO VERIFY THIS PROPERTY OF THE FUNCTION 
    fn decode(buf: &'a [u8]) -> Result<Self, DecodeError> {
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

/// Advert payload type struct, used to update node clock to keep broadcast 
/// timestamps accurate. 
struct Advert<'a> {
    pub public_key: [u8; 32],
    pub timestamp: u32,
    pub signature: [u8; 64],
    pub app_data: &'a [u8],
}

impl<'a> Advert<'a> {
    fn decode(buf: &'a [u8]) -> Result<Self, DecodeError> {
        if buf.len() < 32 + 4 + 64 {
            return Err(DecodeError::TooShort);
        }
        let mut public_key = [0u8; 32];
        public_key.copy_from_slice(&buf[0..32]);
        let timestamp = u32::from_le_bytes(buf[32..36].try_into().unwrap());
        let mut signature = [0u8; 64];
        signature.copy_from_slice(&buf[36..100]);
        Ok(Self { public_key, timestamp, signature, app_data: &buf[100..] })
    }
}

/// Struct used for storing and passing around sensor readings, not all fields are always populated
// NOTE: partial instances are only safe with send_group_text_single_sensor, never send_group_text_sensor_data
#[derive(Clone, Copy, Debug)]
pub struct SensorReadings {
    pub wss: f32,   // May be worth changing these to pub Option<i16> etc, to avoid sending 0's when certain sensors are not polled 
    // pub wds: u16,
    pub aqs: (f64, f64, f64, u8), //(i32, u32, u32, i32),
    pub sms: f32,
    pub tbs: f64,
}

impl SensorReadings {
    const fn empty() -> Self {
        // Self { wss: 0, wds: 0, aqs: (0, 0, 0, 0), sms: 0, tbs: 0.0 }
        Self { wss: 0.0, aqs: (0.0, 0.0, 0.0, 0), sms: 0.0, tbs: 0.0 }
    }
}
impl Default for SensorReadings {
    fn default() -> Self { Self::empty() }
}


//======================================================================================================================
//----Transceiver (frame) level functions that deliver MeshCore packets-------------------------------------------------
//======================================================================================================================

/// (No longer intended to be used) Used for periodic sensor-data broadcast via 
/// RawCustom packet type. For public channelbroadcast see: send_group_text_sensor_data()
fn send_sensor_broadcast_raw_custom(r: &SensorReadings) -> Result<(), EncodeError> {
    let json = Packet::encode_payload(&r)?;
    let response_payload = build_sensor_data_frame(&json)?; 

    let pkt = Packet::originate(RouteType::Flood, PayloadType::RawCustom, &response_payload)?;
    let mut buf = [0u8; MAX_PACKET_LEN];
    let len = pkt.encode(&mut buf)?;
    let mut frame: heapless::Vec<u8, { MAX_PACKET_LEN + 1 }> = heapless::Vec::new();
    frame.extend_from_slice(&buf[..len]).map_err(|_| EncodeError::PayloadTooLong)?;

    if MESHCORE_TX_BUFF.try_send(frame).is_err() {
        warn!("tx queue full, dropping sensor broadcast");
    }
    Ok(())
}

/// Prepend the sub-format tag and package as raw bytes ready for Packet::originate.
fn build_sensor_data_frame(json: &str) -> Result<heapless::Vec<u8, MAX_PAYLOAD_LEN>, EncodeError> {
    let mut buf: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    buf.push(RAW_CUSTOM_RESPONSE_TAG).map_err(|_| EncodeError::PayloadTooLong)?;
    buf.extend_from_slice(json.as_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;
    Ok(buf)
}

/// Send an unencrypted payload to the public channel.
fn send_group_text(text: &str) -> Result<(), EncodeError> {
    let timestamp: u32 = get_current_timestamp();
    let mut plaintext: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    plaintext.extend_from_slice(&timestamp.to_le_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;
    plaintext.push(0x00).map_err(|_| EncodeError::PayloadTooLong)?;
    plaintext.extend_from_slice(text.as_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;

    let mut ciphertext: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    let mac = encrypt_then_mac(&PUBLIC_CHANNEL_KEY, &plaintext, &mut ciphertext)?;

    let mut payload: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    GroupTextEnvelope::encode(channel_hash(&PUBLIC_CHANNEL_KEY), mac, &ciphertext, &mut payload)?;

    let pkt = Packet::originate(RouteType::Flood, PayloadType::GroupText, &payload)?;
    let mut buf = [0u8; MAX_PACKET_LEN];
    let len = pkt.encode(&mut buf)?;
    let mut frame: heapless::Vec<u8, { MAX_PACKET_LEN + 1 }> = heapless::Vec::new();
    frame.extend_from_slice(&buf[..len]).map_err(|_| EncodeError::PayloadTooLong)?;

    if MESHCORE_TX_BUFF.try_send(frame).is_err() {
        warn!("tx queue full, dropping GRP_TXT message");
    }
    Ok(())
}

/// Send all sensor data to the public channel.
pub fn send_group_text_sensor_data(r: &SensorReadings) -> Result<(), EncodeError> {
    // let json = Packet::encode_payload(&r.wss, &r.wds, &r.aqs, &r.sms, &r.tbs)?;
    let json = Packet::encode_payload(&r)?;
    let mut response_text: heapless::String<{ MAX_JSON_LEN + 32 }> = heapless::String::new();
    write!(response_text, "hazard-sensor-{:#04x}: {}", NODE_ID, json).map_err(|_| EncodeError::PayloadTooLong)?;
    send_group_text(&response_text)
}

/// Send requested sensor data to the public channel.
fn send_group_text_single_sensor(key: RequestKey, r: &SensorReadings) -> Result<(), EncodeError> {
    let mut json: heapless::String<64> = heapless::String::new();
    match key {
        RequestKey::Wss => write!(json, "{{\"wss\":{}}}", r.wss),
        // RequestKey::Wds => write!(json, "{{\"wds\":{}}}", r.wds), 
        RequestKey::Aqs => write!(json, "{{\"aqs_tmp\":{},\"aqs_hum\":{},\"aqs_prs\":{},\"aqs_aqi\":{}}}", r.aqs.0, r.aqs.1, r.aqs.2, r.aqs.3),
        RequestKey::Sms => write!(json, "{{\"sms\":{}}}", r.sms), 
        RequestKey::Tbs => write!(json, "{{\"tbs\":{}}}", r.tbs),
        RequestKey::DataAll => return send_group_text_sensor_data(&r),
    }.map_err(|_| EncodeError::PayloadTooLong)?;

    let mut response_text: heapless::String<96> = heapless::String::new();
    write!(response_text, "hazard-sensor-{:#04x}: {}", NODE_ID, json).map_err(|_| EncodeError::PayloadTooLong)?;
    send_group_text(&response_text)
}

/// Current timestamp - for use in outgoing packet plaintexts.
fn get_current_timestamp() -> u32 {
    let elapsed_secs = Instant::now().as_secs() as i32;
    let offset = CLOCK_OFFSET.load(Ordering::Relaxed);
    (elapsed_secs + offset) as u32
}

/// Adjust the clock offset using a timestamp extracted from a received
/// packet (e.g. a request from the phone app, which has real wall-clock
/// time). Only ever moves the clock forward, never backward — a stale or
/// out-of-order packet shouldn't be able to rewind us, since MeshCore
/// timestamps are meant to increase monotonically for dedup/freshness.
fn sync_clock_from_received(received_timestamp: u32) {
    let elapsed_secs = Instant::now().as_secs() as i32;
    let candidate_offset = received_timestamp as i32 - elapsed_secs;
    let current_offset = CLOCK_OFFSET.load(Ordering::Relaxed);
    if candidate_offset > current_offset {
        CLOCK_OFFSET.store(candidate_offset, Ordering::Relaxed);
        CLOCK_SYNCED.store(true, Ordering::Relaxed);
        info!("clock synced from received packet, new offset = {}", candidate_offset);
    }
}

/// Process incoming frame data, react according to packet content.
pub async fn frame_handler(
    raw_frame_data: &[u8],
//     wds: &mut WindDirectionSensor,
    aqs: &mut GasSensor<I2cShared>,
    adc: &mut AdcSensors,
    tbs: &mut RainfallSensor<I2cShared>,
) {
    match Packet::decode(raw_frame_data) {
        Ok(pkt) => {
            // Log received packet info
            info!(
                "MeshCore packet: route={:?} type={:?} hops={} payload_len={}",
                defmt::Debug2Format(&pkt.route_type),
                defmt::Debug2Format(&pkt.payload_type),
                pkt.path.len(),
                pkt.payload.len(),
            );
            
            // Determine action based on rx payload type 
            match pkt.payload_type {
                PayloadType::Request => {
                    info!("Recieved Request packet; packet ignored.");
                    // Can be extended to implement telemetry with MeshCore map
                }  
                PayloadType::Response => {
                    info!("Recieved Response packet; packet ignored.");
                    // Required pair with request 
                }
                PayloadType::TextMessage => {
                    info!("Recieved TextMessage packet; packet ignored.");
                    // Can be extended to handle Text Message packet type for direct message
                }
                PayloadType::Ack => {
                    info!("Recieved Ack packet; packet ignored.");
                }
                // The clock update based on adverts could potentially be abused
                // by malicious advert packets being injected into the network.
                // It is therefore a security risk, but a low one. 
                PayloadType::Advert => match Advert::decode(pkt.payload){
                    Ok(adv) => {
                        sync_clock_from_received(adv.timestamp);
                        info!("Advert received from node with public key: {:?}, clock synced.", adv.public_key);
                    }
                    Err(e) => {
                        error!("Advert decode failed: {:?} - timestamp not updated.", defmt::Debug2Format(&e))
                    }
                }
                PayloadType::GroupText => match GroupTextEnvelope::decode(pkt.payload) {
                    // Match Group Text to public channel or other.
                    Ok(env) if env.channel_hash == channel_hash(&PUBLIC_CHANNEL_KEY) => {
                        let mut plaintext: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
                        // Decrypt HMAC encryption writing to plaintext buffer. 
                        match mac_then_decrypt(&PUBLIC_CHANNEL_KEY, env.mac, env.ciphertext, &mut plaintext) {
                            Ok(()) => {
                                // If recovered plaintext is large enough to hold timestamp, use it to update node clock.
                                if plaintext.len() >= 4 {
                                    let received_ts = u32::from_le_bytes([plaintext[0], plaintext[1], plaintext[2], plaintext[3]]);
                                    sync_clock_from_received(received_ts);
                                }
                                // Match plaintext against known Request Key commands, as specified in section 4.4. of
                                // the LHN prototype development plan.
                                match RequestKey::parse(&plaintext) {
                                    // "DATA" => poll all sensors and send sensor readings. 
                                    Some(RequestKey::DataAll) => {
                                        info!("GRP_TXT DATA request recognised — building response");
                                        let r = poll_all(aqs, adc, tbs).await;
                                        if let Err(e) = send_group_text_sensor_data(&r) {
                                            warn!("failed to build/send data as GRP_TXT to public channel: {:?}", defmt::Debug2Format(&e));
                                        }
                                    }
                                    // other keys => poll sensor that matches request key and send single sensor readings.
                                    Some(key) => {
                                        info!("GRP_TXT single-sensor request recognised: {:?}", defmt::Debug2Format(&key));
                                        let r = poll_req(aqs, adc, tbs, key).await;
                                        if let Err(e) = send_group_text_single_sensor(key, &r) {
                                            warn!("failed to send GRP_TXT single-sensor response: {:?}", defmt::Debug2Format(&e));
                                        }
                                    }
                                    None => info!("GRP_TXT message not a recognised command; ignored"),
                                    }
                            }
                            Err(e) => warn!("GRP_TXT MAC/decrypt failed: {:?}", defmt::Debug2Format(&e)),
                        }
                    }
                    Ok(env) => info!("GRP_TXT on unknown channel (hash={}); ignored", env.channel_hash),
                    Err(e) => warn!("failed to parse GroupText envelope: {:?}", defmt::Debug2Format(&e)),
                }
                
                PayloadType::GroupData => {
                    info!("Recieved GroupData packet; packet ignored.");
                }
                PayloadType::AnonRequest => {
                    info!("Recieved AnonRequest packet; packet ignored.");
                }
                PayloadType::Path => {
                    info!("Recieved Path packet; packet ignored.");
                }
                PayloadType::Trace => {
                    info!("Recieved Trace packet; packet ignored.");
                }
                PayloadType::Multipart => {
                    info!("Recieved Multipart packet; packet ignored.");
                }
                PayloadType::Control => {
                    info!("Recieved Control packet; packet ignored.");
                    // Possibly used with MQTT MeshCore extension for node health checks 
                }
                // Currently unused packet type under intended operation, deprecated by:
                // send_group_text_single_sensor, send_group_text_sensor_data
                PayloadType::RawCustom => {
                    info!("Recieved RawCustom packet; packet ignored.");
                    match pkt.payload.split_first() {
                        Some((&RAW_CUSTOM_REQUEST_TAG, _rest)) => {
                            info!("received request for all sensor data");
                            let r = SensorReadings::default();
                            if let Err(e) = send_sensor_broadcast_raw_custom(&r) {
                                warn!("failed to send RawCustom sensor data: {:?}", defmt::Debug2Format(&e));
                            }
                        }
                        Some((&RAW_CUSTOM_RESPONSE_TAG, rest)) => {
                            if let Ok(json_str) = core::str::from_utf8(rest) {
                                info!("received sensor JSON: {}", json_str);
                            } else {
                                warn!("sensor-data RawCustom payload was not valid UTF-8");
                            }
                        }
                        Some((other, _)) => warn!("RawCustom payload with unknown tag: {}", other),
                        None => warn!("RawCustom payload was empty"),
                    }
                }
            }
        }
        Err(e) => warn!("failed to parse packet in frame_handler: {:?}", defmt::Debug2Format(&e)),
    }
}

/// Send a frame - MeshCore packet inside LoRa frame - may move to network.rs for clarity
pub async fn send_frame(
    radio: &mut SX1262,
    frame: &[u8],
){
    // Ready the radio for transmission with a frame
    if let Err(e) = radio.lora_radio.prepare_for_tx(&radio.mod_params, &mut radio.tx_pkt_params, TX_POWER_DBM, frame)
    .await {
        warn!("prepare_for_tx failed in send_frame: {:?}", defmt::Debug2Format(&e));
        return;
    }
    // Attempt transmission
    if let Err(e) = radio.lora_radio.tx().await {
        warn!("tx failed in send_frame: {:?}", defmt::Debug2Format(&e));
    } else {
        info!("TX successful, {} bytes sent", frame.len())
    }
}

//======================================================================================================================
//----HMAC/AES payload encryption/decryption for group text and text message packet types-------------------------------
//======================================================================================================================

/// GRP_TXT / GRP_DATA payload structure: channel_hash(1) || MAC(2) || ciphertext
struct GroupTextEnvelope<'a> {
    channel_hash: u8,
    mac: [u8; 2],
    ciphertext: &'a [u8],
}

/// Decoding and encoding of GroupTextEnvelope object to and form bytes
impl<'a> GroupTextEnvelope<'a> {
    fn decode(buf: &'a [u8]) -> Result<Self, DecodeError> {
        if buf.len() < 3 {
            return Err(DecodeError::TooShort);
        }
        Ok(Self {
            channel_hash: buf[0],
            mac: [buf[1], buf[2]],
            ciphertext: &buf[3..],
        })
    }

    fn encode(
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
fn channel_hash(key: &[u8]) -> u8 {
    Sha256::digest(key)[0]
}

/// Encrypt plaintext into ciphertext using AES128-ECB encryption.
fn aes128_ecb_encrypt_zero_padded(
    key: &[u8; BLOCK_SIZE],
    plaintext: &[u8],
    out: &mut heapless::Vec<u8, MAX_PAYLOAD_LEN>,
) -> Result<(), EncodeError> {
    out.clear();
    let key_arr = Array::from(*key); 
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

/// Decrypt ciphertext to plaintext by inverse AES128-ECB.
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

/// Compute HMAC encrypted byte (2) given a key and ciphertext.
fn compute_mac(key: &[u8], ciphertext: &[u8]) -> [u8; MAC_LEN] {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(ciphertext);
    let full = mac.finalize().into_bytes();
    let mut truncated = [0u8; MAC_LEN];
    truncated.copy_from_slice(&full[..MAC_LEN]);
    truncated
}

/// Encrypt plaintext into ciphertext using AES128-ECB then apply HMAC the result.
fn encrypt_then_mac(
    key: &[u8; BLOCK_SIZE],
    plaintext: &[u8],
    ciphertext_out: &mut heapless::Vec<u8, MAX_PAYLOAD_LEN>,
) -> Result<[u8; MAC_LEN], EncodeError> {
    aes128_ecb_encrypt_zero_padded(key, plaintext, ciphertext_out)?;
    Ok(compute_mac(key, ciphertext_out))
}

/// Revert HMAC encrypted bytes to retrieve ciphertext, then apply AES128-ECB decyption.
fn mac_then_decrypt(
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


//======================================================================================================================
//----Sensor polling functions------------------------------------------------------------------------------------------
//======================================================================================================================

/// Calls all sensor poll functions to return readings to DATA requests and broadcasts.
pub async fn poll_all(
    // wss: &mut WindSpeedSensor,
    // wds: &mut WindDirectionSensor,
    aqs: &mut GasSensor<I2cShared>,
    adc: &mut AdcSensors,
    tbs: &mut RainfallSensor<I2cShared>,
) -> SensorReadings {
    let wss_data = poll_wss(adc).await;
    // let wds_data = poll_wds(wds).await;
    let aqs_data = poll_aqs(aqs).await;
    let sms_data = poll_sms(adc).await;
    let tbs_data = poll_tbs(tbs).await;

    SensorReadings { wss:wss_data, aqs:aqs_data, sms:sms_data, tbs:tbs_data }
}

/// Poll the requested sensors returning partially populated sensor readings.
async fn poll_req(
    // wss: &mut WindSpeedSensor,
    // wds: &mut WindDirectionSensor,
    aqs: &mut GasSensor<I2cShared>,
    adc: &mut AdcSensors,
    tbs: &mut RainfallSensor<I2cShared>,
    key: RequestKey,
) -> SensorReadings {

    match key {
        RequestKey::Wss => {
            let wss_data = poll_wss(adc).await;
            return SensorReadings { wss:wss_data, aqs:(0.0,0.0,0.0,0), sms:0.0, tbs:0.0 }
            //return SensorReadings { wss:wss_data, aqs:None, sms:None, tbs:None }
        }
        // RequestKey::Wds => write!(json, "{{\"wds\":{}}}", r.wds), 
        RequestKey::Aqs => {
            let aqs_data = poll_aqs(aqs).await;
            return SensorReadings { wss:0.0, aqs:aqs_data, sms:0.0, tbs:0.0 }
            //  return SensorReadings { wss:None, aqs:aqs_data, sms:None, tbs:None }
        }
        RequestKey::Sms => {
            let sms_data = poll_sms(adc).await;
            return SensorReadings { wss:0.0, aqs:(0.0,0.0,0.0,0), sms:sms_data, tbs:0.0 }
            //return SensorReadings { wss:None, aqs:None, sms:sms_data, tbs:None }
        } 
        RequestKey::Tbs => {
            let tbs_data = poll_tbs(tbs).await;
            return SensorReadings { wss:0.0, aqs:(0.0,0.0,0.0,0), sms:0.0, tbs:tbs_data }
            // return SensorReadings { wss:None, aqs:None, sms:None, tbs:tbs_data }
        }
        RequestKey::DataAll => {
            let r = poll_all(aqs, adc, tbs).await;
            return r
        }
    }
}

/// Set of sensor specific polling functions that return the raw data values.
async fn poll_wss(adc: &mut AdcSensors) -> f32 { //i16
    let r = adc.get_soil_moisture().await;
    return r
}
// pub async fn poll_wds(/* wind direction sensor handle */) -> u16 { 
//     // wds.get_wind_direction().await.unwrap()
// }
async fn poll_aqs(aqs: &mut GasSensor<I2cShared>) -> (f64, f64, f64, u8) { // Option<f64>, Option<f64>, Option<f64>, Option<u8>
    match aqs.get_measurements().await{
        Ok(r) => {
            info!("Polled aqs; aqs_tmp:{}, aqs_hum:{}, aqs_prs:{}, aqs_aqi:{}", r.0, r.1, r.2, r.3);
            r
        }
        Err(e) => { 
            error!("Error polling gas sensor: {}.", defmt::Debug2Format(&e));
            (0.0, 0.0, 0.0, 0) // (None, None, None, None)
        }
    }
}
async fn poll_sms(adc: &mut AdcSensors) -> f32 { //i16
    let r = adc.get_soil_moisture().await;
    info!("Polled sms: {}",r);
    return r
}
async fn poll_tbs(tbs: &mut RainfallSensor<I2cShared>) -> f64 { //Option<f64>
    match tbs.get_rainfall().await{
        Ok(r) => { 
            info!("Polled tbs:{}", r);
            r
        }
        Err(e) => { 
            error!("Error polling rainfall sensor: {}.", defmt::Debug2Format(&e));
            0.0 // None
        }
    }
}
