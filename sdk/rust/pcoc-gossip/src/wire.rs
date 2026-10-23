//! Bitcoin P2P framing for the live header source.
//!
//! Block inventory is answered with `getheaders`. This source does not send
//! `getdata` and does not download a block body.

use sha2::{Digest, Sha256};

pub const MAINNET_MAGIC: [u8; 4] = [0xf9, 0xbe, 0xb4, 0xd9];
pub const PROTOCOL_VERSION: i32 = 70_016;
pub const MSG_BLOCK: u32 = 2;
pub const MSG_CMPCT_BLOCK: u32 = 4;
pub const MSG_WITNESS_BLOCK: u32 = 0x4000_0002;
pub const MAX_HEADERS: usize = 2_000;
pub const MAX_INV: usize = 50_000;
pub const MAX_MESSAGE: usize = 4 * 1000 * 1000;

const USER_AGENT: &[u8] = b"/pcoc-bari:0.6.8/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetMessage {
    pub cmd: String,
    pub payload: Vec<u8>,
}

pub fn sha256d(bytes: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(bytes);
    let second = Sha256::digest(first);
    let mut out = [0u8; 32];
    out.copy_from_slice(&second);
    out
}

pub fn encode_message(magic: [u8; 4], cmd: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(24 + payload.len());
    out.extend_from_slice(&magic);
    let mut command = [0u8; 12];
    let raw = cmd.as_bytes();
    let n = raw.len().min(12);
    command[..n].copy_from_slice(&raw[..n]);
    out.extend_from_slice(&command);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    let sum = sha256d(payload);
    out.extend_from_slice(&sum[..4]);
    out.extend_from_slice(payload);
    out
}

/// Take one complete message out of `buf`. A short buffer is `Ok(None)`.
pub fn take_message(magic: [u8; 4], buf: &mut Vec<u8>) -> Result<Option<NetMessage>, ()> {
    if buf.len() < 24 {
        return Ok(None);
    }
    if buf[0..4] != magic {
        return Err(());
    }
    let len = u32::from_le_bytes(buf[16..20].try_into().unwrap()) as usize;
    if len > MAX_MESSAGE {
        return Err(());
    }
    if buf.len() < 24 + len {
        return Ok(None);
    }
    let sum = sha256d(&buf[24..24 + len]);
    if buf[20..24] != sum[..4] {
        return Err(());
    }
    let cmd = command_of(&buf[4..16]);
    let payload = buf[24..24 + len].to_vec();
    buf.drain(..24 + len);
    Ok(Some(NetMessage { cmd, payload }))
}

fn command_of(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

pub fn encode_varint(n: u64) -> Vec<u8> {
    if n < 0xfd {
        vec![n as u8]
    } else if n <= 0xffff {
        let mut v = vec![0xfd];
        v.extend_from_slice(&(n as u16).to_le_bytes());
        v
    } else if n <= 0xffff_ffff {
        let mut v = vec![0xfe];
        v.extend_from_slice(&(n as u32).to_le_bytes());
        v
    } else {
        let mut v = vec![0xff];
        v.extend_from_slice(&n.to_le_bytes());
        v
    }
}

pub fn decode_varint(buf: &[u8]) -> Option<(u64, usize)> {
    if buf.is_empty() {
        return None;
    }
    match buf[0] {
        n @ 0..=0xfc => Some((u64::from(n), 1)),
        0xfd if buf.len() >= 3 => {
            Some((u64::from(u16::from_le_bytes(buf[1..3].try_into().ok()?)), 3))
        }
        0xfe if buf.len() >= 5 => {
            Some((u64::from(u32::from_le_bytes(buf[1..5].try_into().ok()?)), 5))
        }
        0xff if buf.len() >= 9 => Some((u64::from_le_bytes(buf[1..9].try_into().ok()?), 9)),
        _ => None,
    }
}

pub fn encode_version(start_height: i32, now_unix: i64) -> Vec<u8> {
    let mut p = Vec::with_capacity(86 + USER_AGENT.len());
    p.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    p.extend_from_slice(&0u64.to_le_bytes());
    p.extend_from_slice(&now_unix.to_le_bytes());
    write_addr(&mut p, &[0u8; 16], 8333);
    write_addr(&mut p, &[0u8; 16], 0);
    p.extend_from_slice(&1u64.to_le_bytes());
    p.extend_from_slice(&encode_varint(USER_AGENT.len() as u64));
    p.extend_from_slice(USER_AGENT);
    p.extend_from_slice(&start_height.to_le_bytes());
    p.push(0);
    p
}

fn write_addr(p: &mut Vec<u8>, ip: &[u8; 16], port: u16) {
    p.extend_from_slice(&0u64.to_le_bytes());
    p.extend_from_slice(ip);
    p.extend_from_slice(&port.to_be_bytes());
}

pub fn encode_getheaders(locator: &[[u8; 32]], stop: &[u8; 32]) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&(PROTOCOL_VERSION as u32).to_le_bytes());
    p.extend_from_slice(&encode_varint(locator.len() as u64));
    for hash in locator {
        p.extend_from_slice(hash);
    }
    p.extend_from_slice(stop);
    p
}

pub fn decode_getheaders(payload: &[u8]) -> Option<Vec<[u8; 32]>> {
    if payload.len() < 4 {
        return None;
    }
    let (count, mut off) = decode_varint(&payload[4..])?;
    off += 4;
    if count > 128 {
        return None;
    }
    let mut locator = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if payload.len() < off + 32 {
            return None;
        }
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&payload[off..off + 32]);
        locator.push(hash);
        off += 32;
    }
    if payload.len() < off + 32 {
        return None;
    }
    Some(locator)
}

pub fn encode_headers(headers: &[[u8; 80]]) -> Vec<u8> {
    let mut p = encode_varint(headers.len() as u64);
    for header in headers {
        p.extend_from_slice(header);
        p.push(0);
    }
    p
}

pub fn decode_headers(payload: &[u8]) -> Result<Vec<[u8; 80]>, ()> {
    let (count, mut off) = decode_varint(payload).ok_or(())?;
    if count > MAX_HEADERS as u64 {
        return Err(());
    }
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if payload.len() < off + 81 {
            return Err(());
        }
        let mut header = [0u8; 80];
        header.copy_from_slice(&payload[off..off + 80]);
        off += 80;
        let (tx_count, n) = decode_varint(&payload[off..]).ok_or(())?;
        if tx_count != 0 {
            return Err(());
        }
        off += n;
        out.push(header);
    }
    Ok(out)
}

pub fn encode_inv(items: &[(u32, [u8; 32])]) -> Vec<u8> {
    let mut p = encode_varint(items.len() as u64);
    for (kind, hash) in items {
        p.extend_from_slice(&kind.to_le_bytes());
        p.extend_from_slice(hash);
    }
    p
}

pub fn decode_inv(payload: &[u8]) -> Result<Vec<(u32, [u8; 32])>, ()> {
    let (count, mut off) = decode_varint(payload).ok_or(())?;
    if count > MAX_INV as u64 {
        return Err(());
    }
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        if payload.len() < off + 36 {
            return Err(());
        }
        let kind = u32::from_le_bytes(payload[off..off + 4].try_into().unwrap());
        off += 4;
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&payload[off..off + 32]);
        off += 32;
        out.push((kind, hash));
    }
    Ok(out)
}

pub fn is_block_inv(kind: u32) -> bool {
    kind == MSG_BLOCK || kind == MSG_CMPCT_BLOCK || kind == MSG_WITNESS_BLOCK
}

pub fn encode_pong(payload: &[u8]) -> Option<Vec<u8>> {
    if payload.len() < 8 {
        return None;
    }
    Some(payload[..8].to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_payload_round_trips_and_a_bad_checksum_fails() {
        let bytes = encode_message(MAINNET_MAGIC, "verack", &[]);
        let mut buf = bytes.clone();
        let msg = take_message(MAINNET_MAGIC, &mut buf).unwrap().unwrap();
        assert_eq!(msg.cmd, "verack");
        assert!(msg.payload.is_empty());
        assert!(buf.is_empty());
        let mut bad = bytes;
        bad[20] ^= 0xff;
        assert!(take_message(MAINNET_MAGIC, &mut bad).is_err());
    }

    #[test]
    fn headers_and_inv_round_trip() {
        let header = [0x11u8; 80];
        let payload = encode_headers(&[header]);
        let got = decode_headers(&payload).unwrap();
        assert_eq!(got, vec![header]);
        let hash = [0x22u8; 32];
        let inv = encode_inv(&[(MSG_BLOCK, hash), (MSG_WITNESS_BLOCK, hash)]);
        let items = decode_inv(&inv).unwrap();
        assert_eq!(items.len(), 2);
        assert!(is_block_inv(items[0].0));
        assert!(is_block_inv(items[1].0));
        assert!(!is_block_inv(1));
    }
}
