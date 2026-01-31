// Copyright 2025 StrongDM Inc
// SPDX-License-Identifier: Apache-2.0

//! Binary protocol framing and message helpers.

use std::io::{Read, Write};

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use crate::error::{Result, StoreError};

/// Maximum frame payload size (64 MB). Frames larger than this are rejected
/// to prevent memory exhaustion from malicious or corrupted clients.
const MAX_FRAME_SIZE: u32 = 64 * 1024 * 1024;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    Hello = 1,
    CtxCreate = 2,
    CtxFork = 3,
    GetHead = 4,
    AppendTurn = 5,
    GetLast = 6,
    GetBefore = 7,
    GetRangeByDepth = 8,
    GetBlob = 9,
    AttachFs = 10,
    PutBlob = 11,
    Error = 255,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameHeader {
    pub len: u32,
    pub msg_type: u16,
    pub flags: u16,
    pub req_id: u64,
}

#[derive(Debug, Clone)]
pub struct AppendTurnRequest {
    pub context_id: u64,
    pub parent_turn_id: u64,
    pub declared_type_id: String,
    pub declared_type_version: u32,
    pub encoding: u32,
    pub compression: u32,
    pub uncompressed_len: u32,
    pub content_hash: [u8; 32],
    pub payload_bytes: Vec<u8>,
    pub idempotency_key: Vec<u8>,
    /// Optional filesystem snapshot root hash to attach to this turn.
    /// Present if flags bit 0 is set.
    pub fs_root_hash: Option<[u8; 32]>,
}

/// Request to attach a filesystem snapshot to an existing turn.
#[derive(Debug, Clone)]
pub struct AttachFsRequest {
    pub turn_id: u64,
    pub fs_root_hash: [u8; 32],
}

/// Request to store a blob (for filesystem tree objects or file content).
#[derive(Debug, Clone)]
pub struct PutBlobRequest {
    pub hash: [u8; 32],
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
pub struct GetLastRequest {
    pub context_id: u64,
    pub limit: u32,
    pub include_payload: u32,
}

pub fn read_frame<R: Read>(reader: &mut R) -> Result<(FrameHeader, Vec<u8>)> {
    let len = match reader.read_u32::<LittleEndian>() {
        Ok(v) => v,
        Err(e) => return Err(StoreError::Io(e)),
    };

    if len > MAX_FRAME_SIZE {
        return Err(StoreError::InvalidInput(format!(
            "frame size {} exceeds maximum {}",
            len, MAX_FRAME_SIZE
        )));
    }

    let msg_type = reader.read_u16::<LittleEndian>()?;
    let flags = reader.read_u16::<LittleEndian>()?;
    let req_id = reader.read_u64::<LittleEndian>()?;

    let mut payload = vec![0u8; len as usize];
    reader.read_exact(&mut payload)?;
    Ok((
        FrameHeader {
            len,
            msg_type,
            flags,
            req_id,
        },
        payload,
    ))
}

pub fn write_frame<W: Write>(
    writer: &mut W,
    msg_type: u16,
    flags: u16,
    req_id: u64,
    payload: &[u8],
) -> Result<()> {
    writer.write_u32::<LittleEndian>(payload.len() as u32)?;
    writer.write_u16::<LittleEndian>(msg_type)?;
    writer.write_u16::<LittleEndian>(flags)?;
    writer.write_u64::<LittleEndian>(req_id)?;
    writer.write_all(payload)?;
    Ok(())
}

pub fn parse_ctx_create(payload: &[u8]) -> Result<u64> {
    let mut cursor = std::io::Cursor::new(payload);
    Ok(cursor.read_u64::<LittleEndian>()?)
}

pub fn parse_ctx_fork(payload: &[u8]) -> Result<u64> {
    parse_ctx_create(payload)
}

pub fn parse_get_head(payload: &[u8]) -> Result<u64> {
    parse_ctx_create(payload)
}

pub fn parse_get_last(payload: &[u8]) -> Result<GetLastRequest> {
    let mut cursor = std::io::Cursor::new(payload);
    Ok(GetLastRequest {
        context_id: cursor.read_u64::<LittleEndian>()?,
        limit: cursor.read_u32::<LittleEndian>()?,
        include_payload: cursor.read_u32::<LittleEndian>()?,
    })
}

pub fn parse_get_blob(payload: &[u8]) -> Result<[u8; 32]> {
    if payload.len() != 32 {
        return Err(StoreError::InvalidInput("invalid blob hash length".into()));
    }
    let mut hash = [0u8; 32];
    hash.copy_from_slice(payload);
    Ok(hash)
}

pub fn parse_append_turn(payload: &[u8], flags: u16) -> Result<AppendTurnRequest> {
    let mut cursor = std::io::Cursor::new(payload);
    let context_id = cursor.read_u64::<LittleEndian>()?;
    let parent_turn_id = cursor.read_u64::<LittleEndian>()?;

    let type_id_len = cursor.read_u32::<LittleEndian>()? as usize;
    let mut type_id_bytes = vec![0u8; type_id_len];
    cursor.read_exact(&mut type_id_bytes)?;
    let declared_type_id = String::from_utf8(type_id_bytes)
        .map_err(|_| StoreError::InvalidInput("declared_type_id not utf8".into()))?;
    let declared_type_version = cursor.read_u32::<LittleEndian>()?;

    let encoding = cursor.read_u32::<LittleEndian>()?;
    let compression = cursor.read_u32::<LittleEndian>()?;
    let uncompressed_len = cursor.read_u32::<LittleEndian>()?;
    let mut content_hash = [0u8; 32];
    cursor.read_exact(&mut content_hash)?;

    let payload_len = cursor.read_u32::<LittleEndian>()? as usize;
    let mut payload_bytes = vec![0u8; payload_len];
    cursor.read_exact(&mut payload_bytes)?;

    let idempotency_len = cursor.read_u32::<LittleEndian>()? as usize;
    let mut idempotency_key = vec![0u8; idempotency_len];
    if idempotency_len > 0 {
        cursor.read_exact(&mut idempotency_key)?;
    }

    // Check for optional fs_root_hash (flags bit 0)
    let fs_root_hash = if flags & 1 != 0 {
        let mut hash = [0u8; 32];
        cursor.read_exact(&mut hash)?;
        Some(hash)
    } else {
        None
    };

    Ok(AppendTurnRequest {
        context_id,
        parent_turn_id,
        declared_type_id,
        declared_type_version,
        encoding,
        compression,
        uncompressed_len,
        content_hash,
        payload_bytes,
        idempotency_key,
        fs_root_hash,
    })
}

/// Parse ATTACH_FS request: turn_id (u64) + fs_root_hash (32 bytes)
pub fn parse_attach_fs(payload: &[u8]) -> Result<AttachFsRequest> {
    if payload.len() < 40 {
        return Err(StoreError::InvalidInput(
            "attach_fs payload too short".into(),
        ));
    }
    let mut cursor = std::io::Cursor::new(payload);
    let turn_id = cursor.read_u64::<LittleEndian>()?;
    let mut fs_root_hash = [0u8; 32];
    cursor.read_exact(&mut fs_root_hash)?;
    Ok(AttachFsRequest {
        turn_id,
        fs_root_hash,
    })
}

/// Encode ATTACH_FS response: turn_id (u64) + fs_root_hash (32 bytes)
pub fn encode_attach_fs_resp(turn_id: u64, fs_root_hash: &[u8; 32]) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(40);
    buf.write_u64::<LittleEndian>(turn_id)?;
    buf.extend_from_slice(fs_root_hash);
    Ok(buf)
}

/// Parse PUT_BLOB request: hash (32 bytes) + data_len (u32) + data
pub fn parse_put_blob(payload: &[u8]) -> Result<PutBlobRequest> {
    if payload.len() < 36 {
        return Err(StoreError::InvalidInput(
            "put_blob payload too short".into(),
        ));
    }
    let mut cursor = std::io::Cursor::new(payload);
    let mut hash = [0u8; 32];
    cursor.read_exact(&mut hash)?;
    let data_len = cursor.read_u32::<LittleEndian>()? as usize;
    let mut data = vec![0u8; data_len];
    cursor.read_exact(&mut data)?;
    Ok(PutBlobRequest { hash, data })
}

/// Encode PUT_BLOB response: hash (32 bytes) + stored (u8: 1=new, 0=exists)
pub fn encode_put_blob_resp(hash: &[u8; 32], was_new: bool) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(33);
    buf.extend_from_slice(hash);
    buf.push(if was_new { 1 } else { 0 });
    Ok(buf)
}

pub fn encode_ctx_create_resp(
    context_id: u64,
    head_turn_id: u64,
    head_depth: u32,
) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(8 + 8 + 4);
    buf.write_u64::<LittleEndian>(context_id)?;
    buf.write_u64::<LittleEndian>(head_turn_id)?;
    buf.write_u32::<LittleEndian>(head_depth)?;
    Ok(buf)
}

pub fn encode_append_ack(
    context_id: u64,
    new_turn_id: u64,
    new_depth: u32,
    hash: &[u8; 32],
) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(8 + 8 + 4 + 32);
    buf.write_u64::<LittleEndian>(context_id)?;
    buf.write_u64::<LittleEndian>(new_turn_id)?;
    buf.write_u32::<LittleEndian>(new_depth)?;
    buf.extend_from_slice(hash);
    Ok(buf)
}

pub fn encode_error(code: u32, detail: &str) -> Result<Vec<u8>> {
    let mut buf = Vec::new();
    buf.write_u32::<LittleEndian>(code)?;
    buf.write_u32::<LittleEndian>(detail.len() as u32)?;
    buf.extend_from_slice(detail.as_bytes());
    Ok(buf)
}

/// Parsed HELLO request with optional client metadata.
#[derive(Debug, Clone, Default)]
pub struct HelloRequest {
    pub protocol_version: u16,
    pub client_tag: String,
    pub client_meta_json: Option<String>,
}

/// Parse HELLO payload. Supports both old (empty) and new (with metadata) formats.
pub fn parse_hello(payload: &[u8]) -> Result<HelloRequest> {
    // Empty payload = old client, use defaults
    if payload.is_empty() {
        return Ok(HelloRequest::default());
    }

    // New format: protocol_version(u16) + client_tag_len(u16) + client_tag + meta_json_len(u32) + meta_json
    if payload.len() < 4 {
        return Err(StoreError::InvalidInput("hello payload too short".into()));
    }

    let mut cursor = std::io::Cursor::new(payload);
    let protocol_version = cursor.read_u16::<LittleEndian>()?;
    let client_tag_len = cursor.read_u16::<LittleEndian>()? as usize;

    if payload.len() < 4 + client_tag_len + 4 {
        return Err(StoreError::InvalidInput("hello payload truncated".into()));
    }

    let mut client_tag_bytes = vec![0u8; client_tag_len];
    if client_tag_len > 0 {
        cursor.read_exact(&mut client_tag_bytes)?;
    }
    let client_tag = String::from_utf8(client_tag_bytes)
        .map_err(|_| StoreError::InvalidInput("client_tag not utf8".into()))?;

    let client_meta_json_len = cursor.read_u32::<LittleEndian>()? as usize;
    let client_meta_json = if client_meta_json_len > 0 {
        let mut meta_bytes = vec![0u8; client_meta_json_len];
        cursor.read_exact(&mut meta_bytes)?;
        Some(
            String::from_utf8(meta_bytes)
                .map_err(|_| StoreError::InvalidInput("client_meta_json not utf8".into()))?,
        )
    } else {
        None
    };

    Ok(HelloRequest {
        protocol_version,
        client_tag,
        client_meta_json,
    })
}

/// Encode HELLO response with session_id and protocol_version.
pub fn encode_hello_resp(session_id: u64, protocol_version: u16) -> Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(10);
    buf.write_u64::<LittleEndian>(session_id)?;
    buf.write_u16::<LittleEndian>(protocol_version)?;
    Ok(buf)
}
