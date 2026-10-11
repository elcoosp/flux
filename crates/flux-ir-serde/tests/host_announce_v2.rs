//! FLUX-F59 regression: a v2 `HostAnnounce` frame must decode correctly.
//! Before the fix, the decoder built its reader at `PROTOCOL_VERSION`
//! (3), so it read u32 string-length prefixes while the encoder wrote
//! u16 ones — the reader desynced and returned `None`.

use flux_ir_serde::{HostAnnounceFrame, MAGIC, PROTOCOL_VERSION};

const FRAME_HOST_ANNOUNCE: u8 = 0x12;

fn build_v2_bytes(platform: &str, device: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&MAGIC.to_le_bytes());
    bytes.push(2);
    bytes.push(FRAME_HOST_ANNOUNCE);
    bytes.extend_from_slice(&(platform.len() as u16).to_le_bytes());
    bytes.extend_from_slice(platform.as_bytes());
    bytes.extend_from_slice(&(device.len() as u16).to_le_bytes());
    bytes.extend_from_slice(device.as_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes()); // cap_count = 0
    bytes
}

fn build_v3_bytes(platform: &str, device: &str) -> Vec<u8> {
    let mut bytes: Vec<u8> = Vec::new();
    bytes.extend_from_slice(&MAGIC.to_le_bytes());
    bytes.push(PROTOCOL_VERSION);
    bytes.push(FRAME_HOST_ANNOUNCE);
    bytes.extend_from_slice(&(platform.len() as u32).to_le_bytes());
    bytes.extend_from_slice(platform.as_bytes());
    bytes.extend_from_slice(&(device.len() as u32).to_le_bytes());
    bytes.extend_from_slice(device.as_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes()); // cap_count = 0
    bytes
}

#[test]
fn v2_host_announce_frame_decodes() {
    let bytes = build_v2_bytes("ios", "iPhone17,1");
    let decoded = HostAnnounceFrame::from_bytes(&bytes)
        .expect("v2 HostAnnounce frame must decode");
    assert_eq!(decoded.version, 2);
    assert_eq!(decoded.platform, "ios");
    assert_eq!(decoded.device, "iPhone17,1");
    assert!(decoded.capabilities.is_empty());
}

#[test]
fn v3_host_announce_frame_still_decodes() {
    let bytes = build_v3_bytes("android", "Pixel 8");
    let decoded = HostAnnounceFrame::from_bytes(&bytes)
        .expect("v3 HostAnnounce frame must decode");
    assert_eq!(decoded.version, PROTOCOL_VERSION);
    assert_eq!(decoded.platform, "android");
    assert_eq!(decoded.device, "Pixel 8");
}
