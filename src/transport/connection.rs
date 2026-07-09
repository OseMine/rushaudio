use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use crate::protocol::types::{ConnectionState, StreamConfig, StreamStats};


pub struct Connection {
    pub remote_addr: SocketAddr,
    pub state: ConnectionState,
    pub config: StreamConfig,
    pub stats: StreamStats,
    pub sequence_number: AtomicU32,
    pub connected_at: Option<Instant>,
    pub last_activity: Instant,
    pub ssrc: u32,
}

impl Connection {
    pub fn new(remote_addr: SocketAddr) -> Self {
        Self {
            remote_addr,
            state: ConnectionState::Disconnected,
            config: StreamConfig::default(),
            stats: StreamStats::default(),
            sequence_number: AtomicU32::new(0),
            connected_at: None,
            last_activity: Instant::now(),
            ssrc: 0,
        }
    }

    pub fn next_seq(&self) -> u32 {
        self.sequence_number.fetch_add(1, Ordering::SeqCst)
    }

    pub fn current_seq(&self) -> u32 {
        self.sequence_number.load(Ordering::SeqCst)
    }

    pub fn mark_activity(&mut self) {
        self.last_activity = Instant::now();
    }

    pub fn elapsed_since_activity(&self) -> Duration {
        self.last_activity.elapsed()
    }

    pub fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
        if state == ConnectionState::Connected {
            self.connected_at = Some(Instant::now());
        }
    }

    pub fn record_sent(&mut self, bytes: usize) {
        self.stats.packets_sent += 1;
        self.stats.bytes_sent += bytes as u64;
    }

    pub fn record_received(&mut self, bytes: usize) {
        self.stats.packets_received += 1;
        self.stats.bytes_received += bytes as u64;
    }

    pub fn record_loss(&mut self, count: u64) {
        self.stats.packets_lost += count;
    }
}

pub struct ConnectionPool {
    connections: Vec<Connection>,
    max_connections: usize,
}

impl ConnectionPool {
    pub fn new(max: usize) -> Self {
        Self {
            connections: Vec::with_capacity(max),
            max_connections: max,
        }
    }

    pub fn get(&self, addr: SocketAddr) -> Option<&Connection> {
        self.connections.iter().find(|c| c.remote_addr == addr)
    }

    pub fn get_mut(&mut self, addr: SocketAddr) -> Option<&mut Connection> {
        self.connections.iter_mut().find(|c| c.remote_addr == addr)
    }

    pub fn add(&mut self, conn: Connection) -> bool {
        if self.connections.len() >= self.max_connections {
            return false;
        }
        self.connections.push(conn);
        true
    }

    pub fn remove(&mut self, addr: SocketAddr) {
        self.connections.retain(|c| c.remote_addr != addr);
    }

    pub fn len(&self) -> usize {
        self.connections.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Connection> {
        self.connections.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut Connection> {
        self.connections.iter_mut()
    }

    pub fn contains(&self, addr: SocketAddr) -> bool {
        self.connections.iter().any(|c| c.remote_addr == addr)
    }
}
