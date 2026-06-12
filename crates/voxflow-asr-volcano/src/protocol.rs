//! Volcano Engine bidirectional streaming ASR wire format (sauc v3).
//!
//! Frames are binary: a 4-byte header, an optional big-endian i32 sequence,
//! a big-endian u32 payload size, then the payload (JSON or raw audio, both
//! gzip-compressed). Reference: 大模型流式语音识别 API 文档(volcengine
//! docs 6561/1354869)。

use std::io::{Read, Write};

use anyhow::{bail, Context, Result};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::Value;

const PROTOCOL_VERSION: u8 = 0b0001;
const HEADER_WORDS: u8 = 1;
pub const CLIENT_FULL_REQUEST: u8 = 0b0001;
pub const CLIENT_AUDIO_ONLY: u8 = 0b0010;
pub const SERVER_FULL_RESPONSE: u8 = 0b1001;
pub const SERVER_ERROR_RESPONSE: u8 = 0b1111;
const FLAG_POS_SEQUENCE: u8 = 0b0001;
const FLAG_NEG_SEQUENCE: u8 = 0b0011;
const JSON_SERIALIZATION: u8 = 0b0001;
const GZIP_COMPRESSION: u8 = 0b0001;

#[derive(Debug, Default, Clone)]
pub struct ServerMessage {
    pub message_type: u8,
    pub sequence: i32,
    pub is_last_package: bool,
    pub error_code: i32,
    pub payload: Option<Value>,
}

fn header(message_type: u8, flags: u8) -> [u8; 4] {
    [
        (PROTOCOL_VERSION << 4) | HEADER_WORDS,
        (message_type << 4) | flags,
        (JSON_SERIALIZATION << 4) | GZIP_COMPRESSION,
        0,
    ]
}

fn gzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data).context("gzip write")?;
    encoder.finish().context("gzip finish")
}

fn gunzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(data);
    let mut output = Vec::new();
    decoder.read_to_end(&mut output).context("gunzip")?;
    Ok(output)
}

/// First packet: session parameters as gzip JSON, positive sequence.
pub fn encode_full_request(payload: &Value, sequence: i32) -> Result<Vec<u8>> {
    let body = gzip(serde_json::to_string(payload)?.as_bytes())?;
    let mut frame = Vec::with_capacity(12 + body.len());
    frame.extend(header(CLIENT_FULL_REQUEST, FLAG_POS_SEQUENCE));
    frame.extend(sequence.to_be_bytes());
    frame.extend((body.len() as u32).to_be_bytes());
    frame.extend(body);
    Ok(frame)
}

/// Audio packet: gzip PCM bytes; the last packet carries a negative sequence.
pub fn encode_audio_request(pcm: &[u8], sequence: i32, is_last: bool) -> Result<Vec<u8>> {
    let body = gzip(pcm)?;
    let (flags, sequence) = if is_last {
        (FLAG_NEG_SEQUENCE, -sequence)
    } else {
        (FLAG_POS_SEQUENCE, sequence)
    };
    let mut frame = Vec::with_capacity(12 + body.len());
    frame.extend(header(CLIENT_AUDIO_ONLY, flags));
    frame.extend(sequence.to_be_bytes());
    frame.extend((body.len() as u32).to_be_bytes());
    frame.extend(body);
    Ok(frame)
}

pub fn decode_server_message(frame: &[u8]) -> Result<ServerMessage> {
    if frame.len() < 4 {
        bail!("server frame shorter than header");
    }
    let header_size = (frame[0] & 0x0f) as usize * 4;
    if frame.len() < header_size {
        bail!("server frame shorter than declared header");
    }
    let mut message = ServerMessage {
        message_type: frame[1] >> 4,
        ..ServerMessage::default()
    };
    let flags = frame[1] & 0x0f;
    let serialization = frame[2] >> 4;
    let compression = frame[2] & 0x0f;
    let mut offset = header_size;

    let read_i32 = |offset: &mut usize| -> Result<i32> {
        let bytes: [u8; 4] = frame
            .get(*offset..*offset + 4)
            .context("truncated server frame")?
            .try_into()
            .expect("slice length checked");
        *offset += 4;
        Ok(i32::from_be_bytes(bytes))
    };

    if flags & 0x01 != 0 {
        message.sequence = read_i32(&mut offset)?;
    }
    if flags & 0x02 != 0 {
        message.is_last_package = true;
    }
    if message.message_type == SERVER_ERROR_RESPONSE {
        message.error_code = read_i32(&mut offset)?;
    }
    if message.message_type == SERVER_FULL_RESPONSE || message.message_type == SERVER_ERROR_RESPONSE
    {
        let _payload_size = read_i32(&mut offset)?;
    }
    if offset < frame.len() {
        let mut payload = frame[offset..].to_vec();
        if compression == GZIP_COMPRESSION {
            payload = gunzip(&payload)?;
        }
        if serialization == JSON_SERIALIZATION {
            message.payload =
                Some(serde_json::from_slice(&payload).context("server payload json")?);
        }
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn full_request_header_and_sequence() {
        let frame =
            encode_full_request(&json!({"request": {"model_name": "bigmodel"}}), 1).unwrap();
        assert_eq!(frame[0], 0x11);
        assert_eq!(frame[1], (CLIENT_FULL_REQUEST << 4) | 0x01);
        assert_eq!(frame[2], 0x11);
        assert_eq!(i32::from_be_bytes(frame[4..8].try_into().unwrap()), 1);
    }

    #[test]
    fn last_audio_packet_uses_negative_sequence() {
        let frame = encode_audio_request(&[0_u8; 32], 5, true).unwrap();
        assert_eq!(frame[1], (CLIENT_AUDIO_ONLY << 4) | 0b0011);
        assert_eq!(i32::from_be_bytes(frame[4..8].try_into().unwrap()), -5);
    }

    #[test]
    fn server_response_roundtrip() {
        // Hand-build a server frame the same way the service does.
        let payload = json!({"result": {"text": "你好", "utterances": []}});
        let body = gzip(payload.to_string().as_bytes()).unwrap();
        let mut frame = Vec::new();
        frame.extend([0x11, (SERVER_FULL_RESPONSE << 4) | 0x01, 0x11, 0x00]);
        frame.extend(2_i32.to_be_bytes());
        frame.extend((body.len() as u32).to_be_bytes());
        frame.extend(body);

        let message = decode_server_message(&frame).unwrap();
        assert_eq!(message.message_type, SERVER_FULL_RESPONSE);
        assert_eq!(message.sequence, 2);
        assert!(!message.is_last_package);
        assert_eq!(
            message.payload.unwrap()["result"]["text"].as_str(),
            Some("你好")
        );
    }

    #[test]
    fn server_error_response_carries_code() {
        let body = gzip(br#"{"error":"invalid token"}"#).unwrap();
        let mut frame = Vec::new();
        frame.extend([0x11, SERVER_ERROR_RESPONSE << 4, 0x11, 0x00]);
        frame.extend(45000001_i32.to_be_bytes());
        frame.extend((body.len() as u32).to_be_bytes());
        frame.extend(body);

        let message = decode_server_message(&frame).unwrap();
        assert_eq!(message.message_type, SERVER_ERROR_RESPONSE);
        assert_eq!(message.error_code, 45000001);
    }
}
