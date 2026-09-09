use crate::network::{MAX_PACKET_LEN, MAX_PAYLOAD_LEN, PUBLIC_CHANNEL_KEY, Packet, PayloadType, RouteType, EncodeError};
use crate::network;
use crate::MESHCORE_TX_BUFF;
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use defmt::{info, warn};

/// Send a raw custom packet encoding with the request tag to be a data request
pub fn send_request_raw_custom() -> Result<(), EncodeError> {
    // A bare request carries no data — just the tag byte.
    let payload = [network::RAW_CUSTOM_REQUEST_TAG];
    let pkt = Packet::originate(RouteType::Flood, PayloadType::RawCustom, &payload)?;

    let mut buf = [0u8; MAX_PACKET_LEN];
    let len = pkt.encode(&mut buf)?;

    let mut frame: heapless::Vec<u8, { MAX_PACKET_LEN + 1 }> = heapless::Vec::new();
    frame.extend_from_slice(&buf[..len]).map_err(|_| EncodeError::PayloadTooLong)?;

    if MESHCORE_TX_BUFF.try_send(frame).is_err() {
        warn!("tx queue full, dropping request");
    }
    Ok(())
}

/// Send group text packet function, to be used for public channel testing
pub fn send_request_group_text() -> Result<(), EncodeError> {
    let timestamp: u32 = 0; // TODO: real timestamp once a time source exists
    let mut plaintext: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    plaintext.extend_from_slice(&timestamp.to_le_bytes()).map_err(|_| EncodeError::PayloadTooLong)?;
    plaintext.extend_from_slice(b"hazard-sensor: DATA").map_err(|_| EncodeError::PayloadTooLong)?;

    let mut ciphertext: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    let mac = network::encrypt_then_mac(&PUBLIC_CHANNEL_KEY, &plaintext, &mut ciphertext)?;

    let mut payload: heapless::Vec<u8, MAX_PAYLOAD_LEN> = heapless::Vec::new();
    network::GroupTextEnvelope::encode(network::channel_hash(&PUBLIC_CHANNEL_KEY), mac, &ciphertext, &mut payload)?;

    let pkt = Packet::originate(RouteType::Flood, PayloadType::GroupText, &payload)?;
    let mut buf = [0u8; MAX_PACKET_LEN];
    let len = pkt.encode(&mut buf)?;
    let mut frame: heapless::Vec<u8, { MAX_PACKET_LEN + 1 }> = heapless::Vec::new();
    frame.extend_from_slice(&buf[..len]).map_err(|_| EncodeError::PayloadTooLong)?;
    if MESHCORE_TX_BUFF.try_send(frame).is_err() {
        warn!("tx queue full, dropping GRP_TXT request");
    }
    Ok(())
}

