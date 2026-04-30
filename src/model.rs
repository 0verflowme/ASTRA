use anyhow::{bail, Result};

use crate::partition::owner_for_vertex;

pub const DEFAULT_GRID: u64 = 8;
pub const DEFAULT_LANES: u64 = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Op {
    Or = 0,
    Plus = 1,
    Min = 2,
    Max = 3,
}

impl TryFrom<u8> for Op {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Or),
            1 => Ok(Self::Plus),
            2 => Ok(Self::Min),
            3 => Ok(Self::Max),
            _ => bail!("unknown op code {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PacketKey {
    pub dst_chip: u16,
    pub epoch: u16,
    pub op: Op,
    pub block: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Packet {
    pub key: PacketKey,
    pub mask: u64,
    pub value: u64,
    pub pinned: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeTarget {
    Src = 0,
    Dst = 1,
}

impl EdgeTarget {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Src => "src",
            Self::Dst => "dst",
        }
    }
}

impl TryFrom<u8> for EdgeTarget {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Src),
            1 => Ok(Self::Dst),
            _ => bail!("unknown edge target code {value}"),
        }
    }
}

pub fn validate_lanes(lanes: u64) -> Result<()> {
    if (1..=64).contains(&lanes) {
        Ok(())
    } else {
        bail!("lanes must be in 1..=64, got {lanes}")
    }
}

pub fn ensure_phase1_op(op: Op) -> Result<()> {
    match op {
        Op::Or => Ok(()),
        _ => bail!("Phase 1 only supports Op::Or"),
    }
}

pub fn make_bfs_packet(vertex: u64, epoch: u16, lanes: u64, grid: u64) -> Result<Packet> {
    validate_lanes(lanes)?;
    let block = vertex / lanes;
    let lane = vertex % lanes;
    let mask = 1u64 << lane;

    Ok(Packet {
        key: PacketKey {
            dst_chip: owner_for_vertex(vertex, grid)?,
            epoch,
            op: Op::Or,
            block,
        },
        mask,
        value: 0,
        pinned: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanes_must_fit_u64_mask() {
        assert!(validate_lanes(1).is_ok());
        assert!(validate_lanes(64).is_ok());
        assert!(validate_lanes(0).is_err());
        assert!(validate_lanes(65).is_err());
    }

    #[test]
    fn packet_generation_sets_block_and_lane_mask() {
        let packet = make_bfs_packet(130, 7, 64, 8).unwrap();
        assert_eq!(packet.key.epoch, 7);
        assert_eq!(packet.key.op, Op::Or);
        assert_eq!(packet.key.block, 2);
        assert_eq!(packet.mask, 1u64 << 2);
        assert_eq!(packet.value, 0);
        assert!(!packet.pinned);
    }

    #[test]
    fn phase1_rejects_non_or_ops() {
        assert!(ensure_phase1_op(Op::Or).is_ok());
        assert!(ensure_phase1_op(Op::Plus).is_err());
    }
}
