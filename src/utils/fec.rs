use crate::protocol::Packet;

pub struct FecEncoder;

impl FecEncoder {
    /// Generate XOR-based FEC repair packet from a set of data packets.
    /// Simple XOR across all packets in the group.
    pub fn generate_repair(group: &[Packet], seq: u32, ts: u32) -> Option<Packet> {
        if group.is_empty() {
            return None;
        }

        let max_len = group.iter().map(|p| p.payload.len()).max()?;
        let mut repair = vec![0u8; max_len];

        for pkt in group {
            for (i, &byte) in pkt.payload.iter().enumerate() {
                repair[i] ^= byte;
            }
        }

        // Encode FEC metadata: count of packets in group, original sequences
        let mut fec_meta = Vec::with_capacity(2 + group.len() * 4);
        fec_meta.push(group.len() as u8);
        for pkt in group {
            fec_meta.extend_from_slice(&pkt.header.sequence.to_be_bytes());
        }

        let mut payload = Vec::with_capacity(fec_meta.len() + repair.len());
        payload.extend_from_slice(&fec_meta);
        payload.extend_from_slice(&repair);

        Some(Packet::new(
            crate::protocol::types::PacketType::FECData,
            seq,
            ts,
            payload,
        ))
    }

    /// Try to recover a lost packet using the repair packet and available packets.
    /// Returns the recovered payload and its sequence number.
    pub fn try_recover(repair_packet: &Packet, available: &[&Packet]) -> Option<(u32, Vec<u8>)> {
        let payload = &repair_packet.payload;
        if payload.is_empty() {
            return None;
        }

        let group_count = payload[0] as usize;
        let meta_end = 1 + group_count * 4;
        if payload.len() < meta_end {
            return None;
        }

        let mut sequences = Vec::with_capacity(group_count);
        for i in 0..group_count {
            let start = 1 + i * 4;
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(&payload[start..start + 4]);
            sequences.push(u32::from_be_bytes(bytes));
        }

        let repair_data = &payload[meta_end..];

        // Find which sequence is missing
        let lost_seq = {
            let mut missing = None;
            for &seq in &sequences {
                if !available.iter().any(|p| p.header.sequence == seq) {
                    if missing.is_some() {
                        return None; // More than one lost — cannot recover
                    }
                    missing = Some(seq);
                }
            }
            missing?
        };

        // XOR all available packets + repair to recover the lost one
        let mut recovered = repair_data.to_vec();
        for pkt in available {
            if pkt.header.sequence == lost_seq {
                continue;
            }
            if sequences.contains(&pkt.header.sequence) {
                for (i, &byte) in pkt.payload.iter().enumerate() {
                    if i < recovered.len() {
                        recovered[i] ^= byte;
                    }
                }
            }
        }

        Some((lost_seq, recovered))
    }
}
