use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;

use crate::protocol::constants::DEFAULT_KEEPALIVE_INTERVAL_MS;
use crate::protocol::types::{ConnectionState, PacketType, StreamConfig};
use crate::protocol::Packet;
use crate::transport::connection::Connection;

pub struct SessionManager {
    sessions: HashMap<SocketAddr, Session>,
    _keepalive_interval: Duration,
}

pub struct Session {
    pub connection: Connection,
    pub last_keepalive: std::time::Instant,
    pub remote_ssrc: u32,
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: HashMap::new(),
            _keepalive_interval: Duration::from_millis(DEFAULT_KEEPALIVE_INTERVAL_MS),
        }
    }

    pub fn create_session(&mut self, addr: SocketAddr, config: StreamConfig, remote_ssrc: u32) {
        let mut conn = Connection::new(addr);
        conn.config = config;
        conn.ssrc = remote_ssrc;
        conn.set_state(ConnectionState::Connected);

        let session = Session {
            connection: conn,
            last_keepalive: std::time::Instant::now(),
            remote_ssrc,
        };
        self.sessions.insert(addr, session);
    }

    pub fn get_session(&self, addr: &SocketAddr) -> Option<&Session> {
        self.sessions.get(addr)
    }

    pub fn get_session_mut(&mut self, addr: &SocketAddr) -> Option<&mut Session> {
        self.sessions.get_mut(addr)
    }

    pub fn remove_session(&mut self, addr: &SocketAddr) -> Option<Session> {
        self.sessions.remove(addr)
    }

    pub fn has_session(&self, addr: &SocketAddr) -> bool {
        self.sessions.contains_key(addr)
    }

    pub fn send_keepalive(&mut self, addr: &SocketAddr) -> Option<Packet> {
        let session = self.sessions.get(addr)?;
        let seq = session.connection.next_seq();
        let packet = Packet::new(PacketType::KeepAlive, seq, 0, vec![]);
        Some(packet)
    }

    pub fn handle_keepalive(&mut self, addr: &SocketAddr) {
        if let Some(session) = self.sessions.get_mut(addr) {
            session.connection.mark_activity();
            session.last_keepalive = std::time::Instant::now();
        }
    }

    pub fn set_streaming(&mut self, addr: &SocketAddr) {
        if let Some(session) = self.sessions.get_mut(addr) {
            session.connection.set_state(ConnectionState::Streaming);
        }
    }

    pub fn stale_sessions(&self, timeout: Duration) -> Vec<SocketAddr> {
        let now = std::time::Instant::now();
        self.sessions
            .iter()
            .filter(|(_, s)| now.duration_since(s.last_keepalive) > timeout)
            .map(|(addr, _)| *addr)
            .collect()
    }

    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    pub fn all_addrs(&self) -> Vec<SocketAddr> {
        self.sessions.keys().copied().collect()
    }
}
