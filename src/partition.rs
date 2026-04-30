use anyhow::{bail, Result};

use crate::model::PacketKey;

pub fn mix64(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51afd7ed558ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ceb9fe1a85ec53);
    x ^= x >> 33;
    x
}

pub fn validate_grid(grid: u64) -> Result<()> {
    if grid == 0 {
        bail!("grid must be greater than zero");
    }

    let Some(chips) = grid.checked_mul(grid) else {
        bail!("grid is too large: {grid}x{grid} overflows u64");
    };
    if chips > u16::MAX as u64 + 1 {
        bail!("grid is too large for u16 chip IDs: {grid}x{grid}");
    }

    Ok(())
}

pub fn owner_count(grid: u64) -> Result<usize> {
    validate_grid(grid)?;
    Ok((grid * grid) as usize)
}

pub fn chip_id(row: u64, col: u64, grid: u64) -> Result<u16> {
    validate_grid(grid)?;
    if row >= grid || col >= grid {
        bail!("chip coordinate ({row}, {col}) outside {grid}x{grid} grid");
    }
    Ok((row * grid + col) as u16)
}

pub fn owner_for_vertex(v: u64, grid: u64) -> Result<u16> {
    validate_grid(grid)?;
    let row = mix64(v) % grid;
    chip_id(row, 0, grid)
}

pub fn hash_key(key: &PacketKey, stage: u64, sets: usize) -> usize {
    debug_assert!(sets > 0);

    let mut x = key.block;
    x ^= (key.dst_chip as u64) << 48;
    x ^= (key.epoch as u64) << 32;
    x ^= (key.op as u8 as u64) << 24;
    x ^= stage.wrapping_mul(0x9e3779b97f4a7c15);
    (mix64(x) as usize) % sets
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Op, PacketKey};

    #[test]
    fn owner_is_deterministic_and_in_bounds() {
        let a = owner_for_vertex(123_456, 8).unwrap();
        let b = owner_for_vertex(123_456, 8).unwrap();
        assert_eq!(a, b);
        assert!(a < 64);
    }

    #[test]
    fn grid_must_be_positive_and_fit_chip_ids() {
        assert!(validate_grid(0).is_err());
        assert!(validate_grid(8).is_ok());
        assert!(validate_grid(256).is_ok());
        assert!(validate_grid(257).is_err());
    }

    #[test]
    fn switch_hash_is_deterministic() {
        let key = PacketKey {
            dst_chip: 7,
            epoch: 1,
            op: Op::Or,
            block: 99,
        };
        assert_eq!(hash_key(&key, 0, 4096), hash_key(&key, 0, 4096));
        assert!(hash_key(&key, 3, 4096) < 4096);
    }
}
