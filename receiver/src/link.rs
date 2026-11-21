//! Link health: lost packets from gaps in `seq` (PRD requirement B8).

use std::collections::BTreeMap;

use protocol::{Packet, STATUS_FIRST_BOOT};

/// A distance in `seq` from this value up is a step back, not a gap.
const BACKWARD: u16 = 0x8000;

/// Counts the lost packets of one node.
#[derive(Debug, Default)]
pub struct GapCounter {
    last_seq: Option<u16>,
    lost: u32,
}

impl GapCounter {
    /// Count one received packet. Returns the total of lost packets.
    ///
    /// - The `seq` is a u16 and wraps from 65535 to 0.
    /// - A duplicate changes nothing.
    /// - The "first boot" bit or a step back in `seq` means that the node
    ///   restarted. The counter starts again from this `seq` and counts no
    ///   loss for the restart. The step back also finds a restart when the
    ///   packet with the "first boot" bit is lost.
    pub fn record(&mut self, packet: &Packet) -> u32 {
        let restarted = packet.status & STATUS_FIRST_BOOT != 0;
        if let Some(last) = self.last_seq {
            let distance = packet.seq.wrapping_sub(last);
            if !restarted && distance != 0 && distance < BACKWARD {
                self.lost = self.lost.saturating_add(u32::from(distance - 1));
            }
        }
        self.last_seq = Some(packet.seq);
        self.lost
    }
}

/// The gap counters of all nodes.
#[derive(Debug, Default)]
pub struct Links(BTreeMap<u8, GapCounter>);

impl Links {
    /// Count one received packet. Returns the total of lost packets of its node.
    pub fn record(&mut self, packet: &Packet) -> u32 {
        self.0.entry(packet.node_id).or_default().record(packet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn packet(node_id: u8, seq: u16, status: u8) -> Packet {
        Packet {
            node_id,
            seq,
            temp_dc: 200,
            hum_dpct: 500,
            vbat_mv: 3700,
            status,
            prev_retries: 0,
        }
    }

    fn feed(seqs: &[u16]) -> u32 {
        let mut counter = GapCounter::default();
        seqs.iter()
            .map(|seq| counter.record(&packet(1, *seq, 0)))
            .last()
            .unwrap()
    }

    #[test]
    fn consecutive_packets_lose_nothing() {
        assert_eq!(feed(&[10, 11, 12, 13]), 0);
    }

    #[test]
    fn the_first_packet_is_the_baseline() {
        assert_eq!(feed(&[5000]), 0);
    }

    #[test]
    fn a_gap_counts_the_missing_packets() {
        assert_eq!(feed(&[10, 11, 14]), 2);
        assert_eq!(feed(&[10, 12, 15, 16]), 3);
    }

    #[test]
    fn the_count_wraps_with_the_sequence_number() {
        assert_eq!(feed(&[65534, 65535, 0, 1]), 0);
        assert_eq!(feed(&[65534, 1]), 2);
    }

    #[test]
    fn a_duplicate_changes_nothing() {
        assert_eq!(feed(&[10, 11, 11, 12]), 0);
        assert_eq!(feed(&[10, 12, 12, 13]), 1);
    }

    #[test]
    fn the_first_boot_bit_restarts_the_sequence_without_loss() {
        let mut counter = GapCounter::default();
        counter.record(&packet(1, 100, 0));
        counter.record(&packet(1, 103, 0));
        assert_eq!(counter.record(&packet(1, 0, STATUS_FIRST_BOOT)), 2);
        assert_eq!(counter.record(&packet(1, 1, 0)), 2);
        assert_eq!(counter.record(&packet(1, 3, 0)), 3);
    }

    #[test]
    fn a_restart_with_a_higher_sequence_number_loses_nothing() {
        let mut counter = GapCounter::default();
        counter.record(&packet(1, 5, 0));
        assert_eq!(counter.record(&packet(1, 900, STATUS_FIRST_BOOT)), 0);
    }

    #[test]
    fn a_step_back_without_the_first_boot_bit_is_a_restart() {
        assert_eq!(feed(&[500, 501, 2, 3, 5]), 1);
        assert_eq!(feed(&[10, 0, 1]), 0);
    }

    #[test]
    fn nodes_are_counted_apart() {
        let mut links = Links::default();
        links.record(&packet(1, 1, 0));
        links.record(&packet(2, 50, 0));
        assert_eq!(links.record(&packet(1, 4, 0)), 2);
        assert_eq!(links.record(&packet(2, 51, 0)), 0);
    }
}
