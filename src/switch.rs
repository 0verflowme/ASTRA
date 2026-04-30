use anyhow::{bail, Result};

use crate::{
    metrics::Metrics,
    model::{ensure_phase1_op, Packet, PacketKey},
    partition::hash_key,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SwitchConfig {
    pub stages: usize,
    pub sets: usize,
    pub ways: usize,
}

impl SwitchConfig {
    pub fn validate(self) -> Result<Self> {
        if self.stages == 0 {
            bail!("stages must be greater than zero");
        }
        if self.sets == 0 {
            bail!("sets must be greater than zero");
        }
        if self.ways == 0 {
            bail!("ways must be greater than zero");
        }
        self.stages
            .checked_mul(self.sets)
            .and_then(|v| v.checked_mul(self.ways))
            .ok_or_else(|| anyhow::anyhow!("switch table dimensions overflow usize"))?;
        Ok(self)
    }

    pub fn entries(self) -> usize {
        self.stages * self.sets * self.ways
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub key: PacketKey,
    pub mask: u64,
    pub value: u64,
    pub hits: u8,
    pub pinned: bool,
    pub age: u64,
}

impl Entry {
    fn from_packet(packet: Packet, age: u64) -> Self {
        Self {
            key: packet.key,
            mask: packet.mask,
            value: packet.value,
            hits: 0,
            pinned: packet.pinned,
            age,
        }
    }

    fn to_packet(self) -> Packet {
        Packet {
            key: self.key,
            mask: self.mask,
            value: self.value,
            pinned: self.pinned,
        }
    }

    fn score(self) -> (u8, u64) {
        if self.pinned {
            (u8::MAX, u64::MAX)
        } else {
            (self.hits, self.age)
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CarryKind {
    Incoming,
    Evicted,
}

#[derive(Clone, Copy, Debug)]
struct Carry {
    entry: Entry,
    kind: CarryKind,
}

pub struct ReduceSwitch {
    config: SwitchConfig,
    table: Vec<Option<Entry>>,
    clock: u64,
    metrics: Metrics,
}

impl ReduceSwitch {
    pub fn new(config: SwitchConfig, owner_count: usize) -> Result<Self> {
        let config = config.validate()?;
        Ok(Self {
            config,
            table: vec![None; config.entries()],
            clock: 0,
            metrics: Metrics::new(owner_count),
        })
    }

    pub fn process(&mut self, packet: Packet) -> Result<()> {
        ensure_phase1_op(packet.key.op)?;
        self.metrics.record_input();
        self.clock = self.clock.saturating_add(1);

        let mut carry = Carry {
            entry: Entry::from_packet(packet, self.clock),
            kind: CarryKind::Incoming,
        };

        for stage in 0..self.config.stages {
            carry.entry.age = self.clock;
            let set = hash_key(&carry.entry.key, stage as u64, self.config.sets);

            if let Some(hit_index) = self.find_hit(stage, set, carry.entry.key) {
                let resident = self.table[hit_index]
                    .as_mut()
                    .expect("hit index must contain an entry");
                reduce_or(resident, carry.entry)?;
                resident.age = self.clock;
                self.metrics.record_hit();
                return Ok(());
            }

            if let Some(empty_index) = self.find_empty(stage, set) {
                self.table[empty_index] = Some(carry.entry);
                self.metrics.record_admit();
                return Ok(());
            }

            let Some(victim_index) = self.find_victim(stage, set) else {
                continue;
            };
            let victim = self.table[victim_index].expect("victim index must contain an entry");
            if carry.entry.score() > victim.score() {
                self.table[victim_index] = Some(carry.entry);
                carry = Carry {
                    entry: victim,
                    kind: CarryKind::Evicted,
                };
                self.metrics.record_swap();
            }
        }

        match carry.kind {
            CarryKind::Incoming => self.metrics.record_bypass(carry.entry.to_packet()),
            CarryKind::Evicted => self.metrics.record_eviction_flush(carry.entry.to_packet()),
        }
    }

    pub fn drain(&mut self) -> Result<()> {
        for slot in &mut self.table {
            if let Some(entry) = slot.take() {
                self.metrics.record_drain(entry.to_packet())?;
            }
        }
        Ok(())
    }

    pub fn metrics(&self) -> &Metrics {
        &self.metrics
    }

    fn index(&self, stage: usize, set: usize, way: usize) -> usize {
        ((stage * self.config.sets + set) * self.config.ways) + way
    }

    fn find_hit(&self, stage: usize, set: usize, key: PacketKey) -> Option<usize> {
        (0..self.config.ways)
            .map(|way| self.index(stage, set, way))
            .find(|&idx| self.table[idx].is_some_and(|entry| entry.key == key))
    }

    fn find_empty(&self, stage: usize, set: usize) -> Option<usize> {
        (0..self.config.ways)
            .map(|way| self.index(stage, set, way))
            .find(|&idx| self.table[idx].is_none())
    }

    fn find_victim(&self, stage: usize, set: usize) -> Option<usize> {
        (0..self.config.ways)
            .map(|way| self.index(stage, set, way))
            .filter(|&idx| self.table[idx].is_some_and(|entry| !entry.pinned))
            .min_by_key(|&idx| self.table[idx].expect("victim candidate exists").score())
    }
}

fn reduce_or(entry: &mut Entry, incoming: Entry) -> Result<()> {
    ensure_phase1_op(entry.key.op)?;
    ensure_phase1_op(incoming.key.op)?;
    entry.mask |= incoming.mask;
    entry.hits = entry.hits.saturating_add(1);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Op, PacketKey};

    fn packet(block: u64, mask: u64) -> Packet {
        Packet {
            key: PacketKey {
                dst_chip: 0,
                epoch: 1,
                op: Op::Or,
                block,
            },
            mask,
            value: 0,
            pinned: false,
        }
    }

    #[test]
    fn hit_reduces_masks_and_drains_once() {
        let mut switch = ReduceSwitch::new(
            SwitchConfig {
                stages: 1,
                sets: 16,
                ways: 1,
            },
            1,
        )
        .unwrap();

        switch.process(packet(2, 0b001)).unwrap();
        switch.process(packet(2, 0b100)).unwrap();
        switch.drain().unwrap();

        let metrics = switch.metrics();
        assert_eq!(metrics.packets_in, 2);
        assert_eq!(metrics.table_hits, 1);
        assert_eq!(metrics.drained, 1);
        assert_eq!(metrics.packets_out, 1);
        assert_eq!(metrics.packets_out, metrics.packets_out_accounted());
    }

    #[test]
    fn bypass_counts_as_output_pressure() {
        let mut switch = ReduceSwitch::new(
            SwitchConfig {
                stages: 1,
                sets: 1,
                ways: 1,
            },
            1,
        )
        .unwrap();

        switch.process(packet(0, 1)).unwrap();
        switch.process(packet(1, 1)).unwrap();
        switch.drain().unwrap();

        let metrics = switch.metrics();
        assert_eq!(
            metrics.bypassed + metrics.eviction_flushes + metrics.drained,
            metrics.packets_out
        );
        assert_eq!(metrics.owner_queue()[0], metrics.packets_out);
    }

    #[test]
    fn non_or_packet_is_rejected() {
        let mut switch = ReduceSwitch::new(
            SwitchConfig {
                stages: 1,
                sets: 1,
                ways: 1,
            },
            1,
        )
        .unwrap();
        let mut plus = packet(0, 1);
        plus.key.op = Op::Plus;
        assert!(switch.process(plus).is_err());
    }
}
