use std::{
    fs::File,
    io::{BufReader, BufWriter, ErrorKind, Read, Seek, SeekFrom, Write},
    path::Path,
};

use anyhow::{bail, Context, Result};

use crate::model::{validate_lanes, EdgeTarget, Op, Packet, PacketKey};
use crate::partition::validate_grid;

pub const MAGIC: [u8; 8] = *b"ASTRATRC";
pub const VERSION: u16 = 1;
pub const RECORD_SIZE: u16 = 32;
pub const UNKNOWN_PACKET_COUNT: u64 = u64::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TraceHeader {
    pub version: u16,
    pub record_size: u16,
    pub lanes: u64,
    pub grid: u64,
    pub epoch: u16,
    pub target: EdgeTarget,
    pub packet_count: u64,
}

impl TraceHeader {
    pub fn new(lanes: u64, grid: u64, epoch: u16, target: EdgeTarget) -> Result<Self> {
        validate_lanes(lanes)?;
        validate_grid(grid)?;
        Ok(Self {
            version: VERSION,
            record_size: RECORD_SIZE,
            lanes,
            grid,
            epoch,
            target,
            packet_count: UNKNOWN_PACKET_COUNT,
        })
    }

    pub fn validate(self) -> Result<Self> {
        if self.version != VERSION {
            bail!("unsupported trace version {}", self.version);
        }
        if self.record_size != RECORD_SIZE {
            bail!("unsupported trace record_size {}", self.record_size);
        }
        validate_lanes(self.lanes)?;
        validate_grid(self.grid)?;
        Ok(self)
    }
}

pub struct TraceWriter {
    writer: BufWriter<File>,
    header: TraceHeader,
    packet_count: u64,
}

impl TraceWriter {
    pub fn create(path: &Path, header: TraceHeader) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create trace parent {}", parent.display()))?;
        }
        let file =
            File::create(path).with_context(|| format!("create trace {}", path.display()))?;
        let mut writer = BufWriter::new(file);
        write_header_to(&mut writer, header)?;
        Ok(Self {
            writer,
            header,
            packet_count: 0,
        })
    }

    pub fn write_packet(&mut self, packet: Packet) -> Result<()> {
        write_packet_to(&mut self.writer, packet)?;
        self.packet_count = self.packet_count.saturating_add(1);
        Ok(())
    }

    pub fn finish(mut self) -> Result<u64> {
        self.writer.flush()?;
        let mut file = self.writer.into_inner()?;
        file.seek(SeekFrom::Start(0))?;
        self.header.packet_count = self.packet_count;
        write_header_to(&mut file, self.header)?;
        file.flush()?;
        Ok(self.packet_count)
    }
}

pub struct TraceReader {
    reader: BufReader<File>,
    header: TraceHeader,
    packets_read: u64,
}

impl TraceReader {
    pub fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).with_context(|| format!("open trace {}", path.display()))?;
        let mut reader = BufReader::new(file);
        let header = read_header_from(&mut reader)
            .with_context(|| format!("read trace header {}", path.display()))?
            .validate()?;
        Ok(Self {
            reader,
            header,
            packets_read: 0,
        })
    }

    pub fn header(&self) -> TraceHeader {
        self.header
    }

    pub fn read_packet(&mut self) -> Result<Option<Packet>> {
        let packet = read_packet_from(&mut self.reader)?;
        if packet.is_some() {
            self.packets_read = self.packets_read.saturating_add(1);
        }
        Ok(packet)
    }

    pub fn finish(self) -> Result<()> {
        if self.header.packet_count != UNKNOWN_PACKET_COUNT
            && self.packets_read != self.header.packet_count
        {
            bail!(
                "trace packet_count mismatch: header={}, read={}",
                self.header.packet_count,
                self.packets_read
            );
        }
        Ok(())
    }
}

fn write_header_to(mut writer: impl Write, header: TraceHeader) -> Result<()> {
    header.validate()?;
    writer.write_all(&MAGIC)?;
    writer.write_all(&header.version.to_le_bytes())?;
    writer.write_all(&header.record_size.to_le_bytes())?;
    writer.write_all(&header.lanes.to_le_bytes())?;
    writer.write_all(&header.grid.to_le_bytes())?;
    writer.write_all(&header.epoch.to_le_bytes())?;
    writer.write_all(&[header.target.as_u8()])?;
    writer.write_all(&[0; 5])?;
    writer.write_all(&header.packet_count.to_le_bytes())?;
    Ok(())
}

fn read_header_from(mut reader: impl Read) -> Result<TraceHeader> {
    let mut magic = [0u8; 8];
    reader.read_exact(&mut magic)?;
    if magic != MAGIC {
        bail!("invalid trace magic {:?}", magic);
    }

    let version = read_u16(&mut reader)?;
    let record_size = read_u16(&mut reader)?;
    let lanes = read_u64(&mut reader)?;
    let grid = read_u64(&mut reader)?;
    let epoch = read_u16(&mut reader)?;
    let mut target = [0u8; 1];
    reader.read_exact(&mut target)?;
    let target = EdgeTarget::try_from(target[0])?;
    let mut reserved = [0u8; 5];
    reader.read_exact(&mut reserved)?;
    let packet_count = read_u64(&mut reader)?;

    Ok(TraceHeader {
        version,
        record_size,
        lanes,
        grid,
        epoch,
        target,
        packet_count,
    })
}

fn write_packet_to(mut writer: impl Write, packet: Packet) -> Result<()> {
    writer.write_all(&packet.key.dst_chip.to_le_bytes())?;
    writer.write_all(&packet.key.epoch.to_le_bytes())?;
    writer.write_all(&[packet.key.op as u8])?;
    writer.write_all(&[u8::from(packet.pinned)])?;
    writer.write_all(&0u16.to_le_bytes())?;
    writer.write_all(&packet.key.block.to_le_bytes())?;
    writer.write_all(&packet.mask.to_le_bytes())?;
    writer.write_all(&packet.value.to_le_bytes())?;
    Ok(())
}

fn read_packet_from(mut reader: impl Read) -> Result<Option<Packet>> {
    let mut first = [0u8; 1];
    match reader.read_exact(&mut first) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::UnexpectedEof => return Ok(None),
        Err(err) => return Err(err.into()),
    }

    let mut rest = [0u8; RECORD_SIZE as usize - 1];
    reader
        .read_exact(&mut rest)
        .context("partial packet record at end of trace")?;

    let mut record = [0u8; RECORD_SIZE as usize];
    record[0] = first[0];
    record[1..].copy_from_slice(&rest);

    let dst_chip = u16::from_le_bytes([record[0], record[1]]);
    let epoch = u16::from_le_bytes([record[2], record[3]]);
    let op = Op::try_from(record[4])?;
    let flags = record[5];
    let block = u64::from_le_bytes(record[8..16].try_into().expect("slice length"));
    let mask = u64::from_le_bytes(record[16..24].try_into().expect("slice length"));
    let value = u64::from_le_bytes(record[24..32].try_into().expect("slice length"));

    Ok(Some(Packet {
        key: PacketKey {
            dst_chip,
            epoch,
            op,
            block,
        },
        mask,
        value,
        pinned: flags & 0b1 != 0,
    }))
}

fn read_u16(mut reader: impl Read) -> Result<u16> {
    let mut bytes = [0u8; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u64(mut reader: impl Read) -> Result<u64> {
    let mut bytes = [0u8; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn trace_round_trip_preserves_header_and_packet() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("astra_trace_{nonce}.bin"));
        let header = TraceHeader::new(64, 8, 1, EdgeTarget::Dst).unwrap();
        let packet = Packet {
            key: PacketKey {
                dst_chip: 3,
                epoch: 1,
                op: Op::Or,
                block: 22,
            },
            mask: 0b101,
            value: 0,
            pinned: true,
        };

        let mut writer = TraceWriter::create(&path, header).unwrap();
        writer.write_packet(packet).unwrap();
        assert_eq!(writer.finish().unwrap(), 1);

        let mut reader = TraceReader::open(&path).unwrap();
        let read_header = reader.header();
        assert_eq!(read_header.packet_count, 1);
        assert_eq!(read_header.lanes, 64);
        assert_eq!(reader.read_packet().unwrap(), Some(packet));
        assert_eq!(reader.read_packet().unwrap(), None);
        reader.finish().unwrap();
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn trace_header_rejects_bad_lanes() {
        assert!(TraceHeader::new(65, 8, 1, EdgeTarget::Dst).is_err());
    }
}
