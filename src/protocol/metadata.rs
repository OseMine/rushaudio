use std::collections::HashMap;

use crate::protocol::constants::*;
use crate::protocol::types::PacketType;
use crate::protocol::{Packet, PacketError};

/// A single metadata entry: key-value pair.
#[derive(Debug, Clone)]
pub struct MetadataEntry {
    pub key: u8,
    pub value: Vec<u8>,
}

/// Metadata block containing one or more entries.
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    entries: Vec<MetadataEntry>,
}

/// Type alias for convenience building metadata.
pub type MetadataMap = HashMap<u8, Vec<u8>>;

impl Metadata {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_map(map: &MetadataMap) -> Self {
        let mut m = Self::new();
        for (&key, value) in map {
            m.set(key, value.clone());
        }
        m
    }

    pub fn set(&mut self, key: u8, value: Vec<u8>) {
        if let Some(entry) = self.entries.iter_mut().find(|e| e.key == key) {
            entry.value = value;
        } else {
            self.entries.push(MetadataEntry { key, value });
        }
    }

    pub fn set_string(&mut self, key: u8, value: &str) {
        self.set(key, value.as_bytes().to_vec());
    }

    pub fn set_u32(&mut self, key: u8, value: u32) {
        self.set(key, value.to_be_bytes().to_vec());
    }

    pub fn set_u16(&mut self, key: u8, value: u16) {
        self.set(key, value.to_be_bytes().to_vec());
    }

    pub fn get(&self, key: u8) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|e| e.key == key)
            .map(|e| e.value.as_slice())
    }

    pub fn get_string(&self, key: u8) -> Option<String> {
        self.get(key)
            .and_then(|v| std::str::from_utf8(v).ok())
            .map(String::from)
    }

    pub fn get_u32(&self, key: u8) -> Option<u32> {
        self.get(key).and_then(|v| {
            if v.len() >= 4 {
                let mut b = [0u8; 4];
                b.copy_from_slice(&v[..4]);
                Some(u32::from_be_bytes(b))
            } else {
                None
            }
        })
    }

    pub fn get_u16(&self, key: u8) -> Option<u16> {
        self.get(key).and_then(|v| {
            if v.len() >= 2 {
                let mut b = [0u8; 2];
                b.copy_from_slice(&v[..2]);
                Some(u16::from_be_bytes(b))
            } else {
                None
            }
        })
    }

    pub fn remove(&mut self, key: u8) -> bool {
        let len_before = self.entries.len();
        self.entries.retain(|e| e.key != key);
        self.entries.len() < len_before
    }

    pub fn contains(&self, key: u8) -> bool {
        self.entries.iter().any(|e| e.key == key)
    }

    pub fn entries(&self) -> &[MetadataEntry] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&u8, &[u8])> {
        self.entries.iter().map(|e| (&e.key, e.value.as_slice()))
    }

    /// Encode all entries into a byte vector.
    /// Wire format per entry: [key: u8] [length: u16 BE] [value: N bytes]
    pub fn encode(&self) -> Vec<u8> {
        let total_size: usize = self
            .entries
            .iter()
            .map(|e| METADATA_ENTRY_HEADER_SIZE + e.value.len())
            .sum();
        let mut buf = Vec::with_capacity(total_size);
        for entry in &self.entries {
            buf.push(entry.key);
            buf.extend_from_slice(&(entry.value.len() as u16).to_be_bytes());
            buf.extend_from_slice(&entry.value);
        }
        buf
    }

    /// Decode entries from a byte slice.
    pub fn decode(data: &[u8]) -> Result<Self, PacketError> {
        let mut entries = Vec::new();
        let mut offset = 0;

        while offset < data.len() {
            if data.len() - offset < METADATA_ENTRY_HEADER_SIZE {
                return Err(PacketError::InvalidPayload("metadata entry truncated"));
            }

            let key = data[offset];
            offset += 1;

            let mut len_bytes = [0u8; 2];
            len_bytes.copy_from_slice(&data[offset..offset + 2]);
            let value_len = u16::from_be_bytes(len_bytes) as usize;
            offset += 2;

            if offset + value_len > data.len() {
                return Err(PacketError::InvalidPayload("metadata value truncated"));
            }

            let value = data[offset..offset + value_len].to_vec();
            offset += value_len;

            entries.push(MetadataEntry { key, value });
        }

        Ok(Self { entries })
    }

    /// Wrap into a Packet with PacketType::Metadata.
    pub fn to_packet(&self, seq: u32, ts: u32) -> Packet {
        Packet::new(PacketType::Metadata, seq, ts, self.encode())
    }

    /// Extract Metadata from a Packet.
    pub fn from_packet(packet: &Packet) -> Result<Self, PacketError> {
        if packet.header.packet_type != PacketType::Metadata {
            return Err(PacketError::InvalidPayload("not a metadata packet"));
        }
        Self::decode(&packet.payload)
    }
}

/// Convenience builder for common metadata fields.
pub struct MetadataBuilder {
    meta: Metadata,
}

impl MetadataBuilder {
    pub fn new() -> Self {
        Self {
            meta: Metadata::new(),
        }
    }

    pub fn ssrc(mut self, ssrc: u32) -> Self {
        self.meta.set_u32(META_SSRC, ssrc);
        self
    }

    pub fn track_title(mut self, title: &str) -> Self {
        self.meta.set_string(META_TRACK_TITLE, title);
        self
    }

    pub fn artist(mut self, artist: &str) -> Self {
        self.meta.set_string(META_ARTIST, artist);
        self
    }

    pub fn album(mut self, album: &str) -> Self {
        self.meta.set_string(META_ALBUM, album);
        self
    }

    pub fn genre(mut self, genre: &str) -> Self {
        self.meta.set_string(META_GENRE, genre);
        self
    }

    pub fn sample_rate(mut self, rate: u32) -> Self {
        self.meta.set_u32(META_SAMPLE_RATE, rate);
        self
    }

    pub fn channels(mut self, channels: u16) -> Self {
        self.meta.set_u16(META_CHANNELS, channels);
        self
    }

    pub fn codec_info(mut self, info: &str) -> Self {
        self.meta.set_string(META_CODEC_INFO, info);
        self
    }

    pub fn bitrate(mut self, bitrate: u32) -> Self {
        self.meta.set_u32(META_BITRATE, bitrate);
        self
    }

    pub fn duration_ms(mut self, duration: u64) -> Self {
        self.meta
            .set(key_u64(META_DURATION_MS), duration.to_be_bytes().to_vec());
        self
    }

    pub fn stream_title(mut self, title: &str) -> Self {
        self.meta.set_string(META_STREAM_TITLE, title);
        self
    }

    pub fn stream_url(mut self, url: &str) -> Self {
        self.meta.set_string(META_STREAM_URL, url);
        self
    }

    pub fn custom(mut self, key: u8, value: Vec<u8>) -> Self {
        if key < META_CUSTOM_BASE {
            panic!("custom metadata key must be >= 0x80");
        }
        self.meta.set(key, value);
        self
    }

    pub fn build(self) -> Metadata {
        self.meta
    }
}

fn key_u64(key: u8) -> u8 {
    key
}

impl Default for MetadataBuilder {
    fn default() -> Self {
        Self::new()
    }
}
