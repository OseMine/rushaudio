use std::net::SocketAddr;
use std::time::{Duration, Instant};

use crate::protocol::constants::*;
use crate::protocol::types::{AudioCodec, PacketType, StreamConfig};
use crate::protocol::Packet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeRole {
    Initiator,
    Responder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandshakeState {
    Idle,
    WaitingForResponse,
    WaitingForRequest,
    Completed,
    Failed,
}

pub struct Handshake {
    pub role: HandshakeRole,
    pub state: HandshakeState,
    pub remote_addr: Option<SocketAddr>,
    pub ssrc: u32,
    pub config: StreamConfig,
    pub started_at: Option<Instant>,
    pub timeout: Duration,
}

impl Handshake {
    pub fn new(role: HandshakeRole, ssrc: u32, config: StreamConfig) -> Self {
        Self {
            role,
            state: HandshakeState::Idle,
            remote_addr: None,
            ssrc,
            config,
            started_at: None,
            timeout: Duration::from_millis(DEFAULT_HANDSHAKE_TIMEOUT_MS),
        }
    }

    pub fn build_request(&self, seq: u32, ts: u32) -> Packet {
        let mut payload = Vec::with_capacity(9);
        payload.extend_from_slice(&self.ssrc.to_be_bytes());
        payload.extend_from_slice(&self.config.sample_rate.to_be_bytes());
        payload.extend_from_slice(&self.config.bitrate.to_be_bytes());
        payload.push(self.config.codec.to_u8());
        payload.push(self.config.channels as u8);
        Packet::new(PacketType::HandshakeRequest, seq, ts, payload)
    }

    pub fn build_response(&self, seq: u32, ts: u32, accepted: bool) -> Packet {
        let mut payload = vec![if accepted { 0x01 } else { 0x00 }];
        payload.extend_from_slice(&self.ssrc.to_be_bytes());
        Packet::new(PacketType::HandshakeResponse, seq, ts, payload)
    }

    pub fn parse_request(packet: &Packet) -> Option<(u32, StreamConfig)> {
        let payload = &packet.payload;
        if payload.len() < 12 {
            return None;
        }
        let mut ssrc_bytes = [0u8; 4];
        ssrc_bytes.copy_from_slice(&payload[0..4]);
        let remote_ssrc = u32::from_be_bytes(ssrc_bytes);

        let mut sr_bytes = [0u8; 4];
        sr_bytes.copy_from_slice(&payload[4..8]);
        let sample_rate = u32::from_be_bytes(sr_bytes);

        let mut br_bytes = [0u8; 4];
        br_bytes.copy_from_slice(&payload[8..12]);
        let bitrate = u32::from_be_bytes(br_bytes);

        let codec = AudioCodec::from_u8(*payload.get(12).unwrap_or(&0)).unwrap_or(AudioCodec::Opus);
        let channels = *payload.get(13).unwrap_or(&2) as u16;

        Some((
            remote_ssrc,
            StreamConfig {
                sample_rate,
                channels,
                bitrate,
                codec,
                ..Default::default()
            },
        ))
    }

    pub fn parse_response(packet: &Packet) -> Option<(bool, u32)> {
        if packet.payload.len() < 5 {
            return None;
        }
        let accepted = packet.payload[0] == 0x01;

        let mut ssrc_bytes = [0u8; 4];
        ssrc_bytes.copy_from_slice(&packet.payload[1..5]);
        let ssrc = u32::from_be_bytes(ssrc_bytes);
        Some((accepted, ssrc))
    }

    pub fn start(&mut self, remote: SocketAddr) {
        self.state = HandshakeState::WaitingForResponse;
        self.remote_addr = Some(remote);
        self.started_at = Some(Instant::now());
    }

    pub fn timed_out(&self) -> bool {
        match self.started_at {
            Some(start) => start.elapsed() > self.timeout,
            None => true,
        }
    }
}
