use anyhow::{bail, Result};

use crate::model::Packet;

#[derive(Debug, Clone)]
pub struct Metrics {
    pub packets_in: u64,
    pub packets_out: u64,
    pub table_hits: u64,
    pub admitted: u64,
    pub bypassed: u64,
    pub eviction_swaps: u64,
    pub eviction_flushes: u64,
    pub drained: u64,
    owner_queue: Vec<u64>,
}

impl Metrics {
    pub fn new(owner_count: usize) -> Self {
        Self {
            packets_in: 0,
            packets_out: 0,
            table_hits: 0,
            admitted: 0,
            bypassed: 0,
            eviction_swaps: 0,
            eviction_flushes: 0,
            drained: 0,
            owner_queue: vec![0; owner_count],
        }
    }

    pub fn record_input(&mut self) {
        self.packets_in = self.packets_in.saturating_add(1);
    }

    pub fn record_hit(&mut self) {
        self.table_hits = self.table_hits.saturating_add(1);
    }

    pub fn record_admit(&mut self) {
        self.admitted = self.admitted.saturating_add(1);
    }

    pub fn record_swap(&mut self) {
        self.eviction_swaps = self.eviction_swaps.saturating_add(1);
    }

    pub fn record_bypass(&mut self, packet: Packet) -> Result<()> {
        self.bypassed = self.bypassed.saturating_add(1);
        self.emit_to_owner(packet)
    }

    pub fn record_eviction_flush(&mut self, packet: Packet) -> Result<()> {
        self.eviction_flushes = self.eviction_flushes.saturating_add(1);
        self.emit_to_owner(packet)
    }

    pub fn record_drain(&mut self, packet: Packet) -> Result<()> {
        self.drained = self.drained.saturating_add(1);
        self.emit_to_owner(packet)
    }

    fn emit_to_owner(&mut self, packet: Packet) -> Result<()> {
        let dst = packet.key.dst_chip as usize;
        let Some(queue) = self.owner_queue.get_mut(dst) else {
            bail!(
                "dst_chip {} outside owner queue length {}",
                packet.key.dst_chip,
                self.owner_queue.len()
            );
        };
        *queue = queue.saturating_add(1);
        self.packets_out = self.packets_out.saturating_add(1);
        Ok(())
    }

    pub fn hit_rate(&self) -> f64 {
        self.table_hits as f64 / self.packets_in.max(1) as f64
    }

    pub fn bypass_rate(&self) -> f64 {
        self.bypassed as f64 / self.packets_in.max(1) as f64
    }

    pub fn compression(&self) -> f64 {
        self.packets_in as f64 / self.packets_out.max(1) as f64
    }

    pub fn owner_queue_max(&self) -> u64 {
        self.owner_queue.iter().copied().max().unwrap_or(0)
    }

    pub fn owner_queue_mean(&self) -> f64 {
        if self.owner_queue.is_empty() {
            0.0
        } else {
            self.packets_out as f64 / self.owner_queue.len() as f64
        }
    }

    pub fn packets_out_accounted(&self) -> u64 {
        self.bypassed + self.eviction_flushes + self.drained
    }

    pub fn owner_queue(&self) -> &[u64] {
        &self.owner_queue
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Op, PacketKey};

    fn packet(dst_chip: u16) -> Packet {
        Packet {
            key: PacketKey {
                dst_chip,
                epoch: 1,
                op: Op::Or,
                block: 0,
            },
            mask: 1,
            value: 0,
            pinned: false,
        }
    }

    #[test]
    fn packets_out_is_accounted_by_output_causes() {
        let mut metrics = Metrics::new(4);
        metrics.record_bypass(packet(0)).unwrap();
        metrics.record_eviction_flush(packet(1)).unwrap();
        metrics.record_drain(packet(2)).unwrap();

        assert_eq!(metrics.packets_out, 3);
        assert_eq!(metrics.packets_out, metrics.packets_out_accounted());
        assert_eq!(metrics.owner_queue_max(), 1);
        assert_eq!(metrics.owner_queue_mean(), 0.75);
    }
}
