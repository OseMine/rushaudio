use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::protocol::constants::DEFAULT_JITTER_BUFFER_MS;

pub struct JitterBuffer {
    packets: VecDeque<JitterPacket>,
    capacity: usize,
    target_delay: Duration,
    min_delay: Duration,
    max_delay: Duration,
    last_playout_ts: Option<u32>,
    dropped_packets: u64,
    inserted_packets: u64,
    late_packets: u64,
    current_jitter: f64,
}

struct JitterPacket {
    data: Vec<u8>,
    sequence: u32,
    timestamp: u32,
    received_at: Instant,
}

impl JitterBuffer {
    pub fn new() -> Self {
        let target = Duration::from_millis(DEFAULT_JITTER_BUFFER_MS);
        Self {
            packets: VecDeque::with_capacity(256),
            capacity: 256,
            target_delay: target,
            min_delay: Duration::from_millis(20),
            max_delay: Duration::from_millis(400),
            last_playout_ts: None,
            dropped_packets: 0,
            inserted_packets: 0,
            late_packets: 0,
            current_jitter: 0.0,
        }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            packets: VecDeque::with_capacity(cap),
            capacity: cap,
            ..Self::new()
        }
    }

    pub fn push(&mut self, sequence: u32, timestamp: u32, data: Vec<u8>) {
        if self.packets.len() >= self.capacity {
            // Drop oldest if full
            self.packets.pop_front();
            self.dropped_packets += 1;
        }

        // Insert in sequence order
        let packet = JitterPacket {
            data,
            sequence,
            timestamp,
            received_at: Instant::now(),
        };

        let pos = self
            .packets
            .binary_search_by(|p| p.sequence.cmp(&sequence));
        match pos {
            Ok(_) => {} // Duplicate, drop
            Err(idx) => {
                // Check if too late (gap calculation)
                if let Some(last_ts) = self.last_playout_ts {
                    if sequence < last_ts && (last_ts - sequence) > 1000 {
                        self.late_packets += 1;
                        return;
                    }
                }
                self.packets.insert(idx, packet);
            }
        }
    }

    pub fn pop(&mut self) -> Option<(u32, Vec<u8>)> {
        let now = Instant::now();
        if self.packets.is_empty() {
            return None;
        }

        let head = &self.packets[0];

        // Wait until we have accumulated target delay worth of packets
        let head_age = now.duration_since(head.received_at);
        if head_age < self.target_delay {
            return None;
        }

        let packet = self.packets.pop_front()?;
        self.last_playout_ts = Some(packet.timestamp);

        // Adaptive jitter estimation
        let age_ms = head_age.as_secs_f64() * 1000.0;
        self.current_jitter = self.current_jitter * 0.875 + age_ms * 0.125;

        Some((packet.timestamp, packet.data))
    }

    pub fn peek(&self) -> Option<(u32, u32, usize)> {
        self.packets
            .front()
            .map(|p| (p.sequence, p.timestamp, p.data.len()))
    }

    pub fn len(&self) -> usize {
        self.packets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.packets.is_empty()
    }

    pub fn clear(&mut self) {
        self.packets.clear();
    }

    pub fn stats(&self) -> JitterStats {
        JitterStats {
            depth: self.packets.len(),
            dropped: self.dropped_packets,
            inserted: self.inserted_packets,
            late: self.late_packets,
            jitter_ms: self.current_jitter,
            target_delay_ms: self.target_delay.as_secs_f64() * 1000.0,
        }
    }

    pub fn adapt_delay(&mut self) {
        // Dynamically adjust target delay based on observed jitter
        let jitter_ms = self.current_jitter;
        let new_target = Duration::from_secs_f64((jitter_ms * 2.0 + 10.0).max(20.0) / 1000.0);
        self.target_delay = new_target.clamp(self.min_delay, self.max_delay);
    }

    pub fn set_min_delay(&mut self, ms: u64) {
        self.min_delay = Duration::from_millis(ms);
    }

    pub fn set_max_delay(&mut self, ms: u64) {
        self.max_delay = Duration::from_millis(ms);
    }
}

#[derive(Debug, Clone)]
pub struct JitterStats {
    pub depth: usize,
    pub dropped: u64,
    pub inserted: u64,
    pub late: u64,
    pub jitter_ms: f64,
    pub target_delay_ms: f64,
}
