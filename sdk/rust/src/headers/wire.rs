//! Bitcoin P2P framing for the header source. Hashes on the wire are natural order,
//! the same bytes as `header_hash` (SPEC §9 P9.17). Block `inv` is answered with
//! `getheaders`, not `getdata`.

pub const PROTOCOL_VERSION: i32 = 70_016;
pub const MSG_BLOCK: u32 = 2;
pub const MSG_CMPCT_BLOCK: u32 = 4;
pub const MSG_WITNESS_BLOCK: u32 = 0x4000_0002;
pub const MAX_HEADERS: usize = 2_000;
pub const MAX_INV: usize = 50_000;
/// Bitcoin `MAX_PROTOCOL_MESSAGE_LENGTH`.
pub const MAX_MESSAGE: usize = 4 * 1000 * 1000;

const USER_AGENT: &str = "/pcoc-sdk:2.2.0/";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetMessage {
    pub cmd: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetHeaders {
    pub version: u32,
    pub locator: Vec<[u8; 32]>,
    pub stop: [u8; 32],
}

pub fn encode_message(magic: [u8; 4], cmd: &str, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(24 + payload.len());
    out.extend_from_slice(&magic);
    let mut command = [0u8; 12];
    let bytes = cmd.as_bytes();
    let n = bytes.len().min(12);
    command[..n].copy_from_slice(&bytes[..n]);
    out.extend_from_slice(&command);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    let sum = header_hash_raw(payload);
    out.extend_from_slice(&sum[..4]);
    out.extend_from_slice(payload);
    out
}

fn header_hash_raw(payload: &[u8]) -> [u8; 32] {
    use crate::hash::sha256;
    sha256(&sha256(payload))
}

/// Pull every complete message out of `buf`. A short buffer returns `Ok` with
/// the remainder still in `rest`. A bad magic, checksum, or length is `Err`.
pub fn drain_messages(magic: [u8; 4], mut buf: Vec<u8>) -> Result<(Vec<NetMessage>, Vec<u8>), ()> {
    let mut msgs = Vec::new();
    loop {
        if buf.len() < 24 {
            break;
        }
        if buf[0..4] != magic {
            return Err(());
        }
        let len = u32::from_le_bytes(buf[16..20].try_into().unwrap()) as usize;
        if len > MAX_MESSAGE {
            return Err(());
        }
        if buf.len() < 24 + len {
            break;
        }
        let sum = header_hash_raw(&buf[24..24 + len]);
        if buf[20..24] != sum[..4] {
            return Err(());
        }
        let cmd = command_of(&buf[4..16]);
        let payload = buf[24..24 + len].to_vec();
        buf.drain(..24 + len);
        msgs.push(NetMessage { cmd, payload });
    }
    Ok((msgs, buf))
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
        0xfd => {
            if buf.len() < 3 {
                return None;
            }
            Some((u64::from(u16::from_le_bytes(buf[1..3].try_into().ok()?)), 3))
        }
        0xfe => {
            if buf.len() < 5 {
                return None;
            }
            Some((u64::from(u32::from_le_bytes(buf[1..5].try_into().ok()?)), 5))
        }
        0xff => {
            if buf.len() < 9 {
                return None;
            }
            Some((u64::from_le_bytes(buf[1..9].try_into().ok()?), 9))
        }
    }
}

pub fn encode_version(start_height: i32, nonce: u64, now_unix: i64) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&PROTOCOL_VERSION.to_le_bytes());
    p.extend_from_slice(&0u64.to_le_bytes());
    p.extend_from_slice(&now_unix.to_le_bytes());
    write_addr(&mut p, 0, &[0u8; 16], 0);
    write_addr(&mut p, 0, &[0u8; 16], 0);
    p.extend_from_slice(&nonce.to_le_bytes());
    write_varstr(&mut p, USER_AGENT.as_bytes());
    p.extend_from_slice(&start_height.to_le_bytes());
    p.push(0); // relay transactions: no. Blocks are still announced.
    p
}

fn write_addr(p: &mut Vec<u8>, services: u64, ip: &[u8; 16], port: u16) {
    p.extend_from_slice(&services.to_le_bytes());
    p.extend_from_slice(ip);
    p.extend_from_slice(&port.to_be_bytes());
}

fn write_varstr(p: &mut Vec<u8>, bytes: &[u8]) {
    p.extend_from_slice(&encode_varint(bytes.len() as u64));
    p.extend_from_slice(bytes);
}

pub fn encode_ping(nonce: u64) -> Vec<u8> {
    nonce.to_le_bytes().to_vec()
}

pub fn decode_ping(payload: &[u8]) -> Option<u64> {
    if payload.len() < 8 {
        return None;
    }
    Some(u64::from_le_bytes(payload[..8].try_into().ok()?))
}

pub fn encode_getheaders(version: u32, locator: &[[u8; 32]], stop: &[u8; 32]) -> Vec<u8> {
    let mut p = Vec::new();
    p.extend_from_slice(&version.to_le_bytes());
    p.extend_from_slice(&encode_varint(locator.len() as u64));
    for hash in locator {
        p.extend_from_slice(hash);
    }
    p.extend_from_slice(stop);
    p
}

pub fn decode_getheaders(payload: &[u8]) -> Option<GetHeaders> {
    if payload.len() < 4 {
        return None;
    }
    let version = u32::from_le_bytes(payload[..4].try_into().ok()?);
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
    let mut stop = [0u8; 32];
    stop.copy_from_slice(&payload[off..off + 32]);
    Some(GetHeaders {
        version,
        locator,
        stop,
    })
}

pub fn encode_headers(headers: &[[u8; 80]]) -> Vec<u8> {
    let mut p = encode_varint(headers.len() as u64);
    for h in headers {
        p.extend_from_slice(h);
        p.push(0); // txn count
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
        if payload.len() < off + 80 {
            return Err(());
        }
        let mut header = [0u8; 80];
        header.copy_from_slice(&payload[off..off + 80]);
        off += 80;
        let (tx, n) = decode_varint(&payload[off..]).ok_or(())?;
        if tx != 0 {
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

/// One `addr` entry. `ip` is 16 bytes; IPv4 uses the `::ffff:0:0/96` prefix.
pub fn encode_addr(entries: &[([u8; 16], u16)]) -> Vec<u8> {
    let mut p = encode_varint(entries.len() as u64);
    for (ip, port) in entries {
        p.extend_from_slice(&0u32.to_le_bytes());
        p.extend_from_slice(&1u64.to_le_bytes());
        p.extend_from_slice(ip);
        p.extend_from_slice(&port.to_be_bytes());
    }
    p
}

pub fn decode_addr(payload: &[u8]) -> Result<Vec<([u8; 16], u16)>, ()> {
    let (count, mut off) = decode_varint(payload).ok_or(())?;
    if count > 1000 {
        return Err(());
    }
    let mut out = Vec::with_capacity(count as usize);
    for _ in 0..count {
        // time u32 + services u64 + ip 16 + port 2
        if payload.len() < off + 30 {
            return Err(());
        }
        off += 4 + 8;
        let mut ip = [0u8; 16];
        ip.copy_from_slice(&payload[off..off + 16]);
        off += 16;
        let port = u16::from_be_bytes(payload[off..off + 2].try_into().unwrap());
        off += 2;
        out.push((ip, port));
    }
    Ok(out)
}

pub fn is_block_inv(kind: u32) -> bool {
    kind == MSG_BLOCK || kind == MSG_CMPCT_BLOCK || kind == MSG_WITNESS_BLOCK
}
