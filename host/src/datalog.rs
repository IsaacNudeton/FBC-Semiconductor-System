//! Binary Datalog Writer/Reader
//!
//! Captures raw FBC packets to a compact binary file during test execution.
//! The packet IS the record — zero additional serialization.
//!
//! # Format
//!
//! ```text
//! HEADER (32 bytes, once):
//!   [0:4]   magic = "FBD\x01" (FBC Datalog v1)
//!   [4]     version = 1
//!   [5:11]  board_mac (6 bytes)
//!   [11:19] test_start_epoch (u64 LE, seconds since Unix epoch)
//!   [19:23] plan_hash (u32 LE, CRC of plan definition)
//!   [23:32] reserved (9 bytes, zero)
//!
//! BODY (repeating, variable-length):
//!   [0:4]   offset_ms (u32 LE, ms since test_start)
//!   [4:12]  fbc_header (8 bytes, raw FBC protocol header)
//!   [12:N]  payload (fbc_header.length bytes)
//!
//! FOOTER (40 bytes, once):
//!   [0:4]   record_count (u32 LE)
//!   [4:8]   body_crc32 (u32 LE, CRC over all body records)
//!   [8:72]  min_max (32 channels × 2 × u16 = 128 bytes... too big)
//!   ... actually keep footer minimal:
//!   [0:4]   record_count (u32 LE)
//!   [4:8]   body_crc32 (u32 LE)
//!   [8:12]  end_magic = "FBD\xFF"
//! ```
//!
//! At 1 sample/sec for 500 hours: 1.8M records × ~59 bytes avg = ~106 MB.
//! Sonoma CSV for same data: 400+ MB. ~4x denser, lossless, random-accessible.

use std::io::{self, Write, Read, Seek, SeekFrom, BufWriter, BufReader};
use std::fs::File;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::fbc_protocol::FbcPacket;

/// Datalog file magic
const MAGIC: [u8; 4] = [b'F', b'B', b'D', 0x01];
/// Footer end magic
const END_MAGIC: [u8; 4] = [b'F', b'B', b'D', 0xFF];
/// Header size
const HEADER_SIZE: usize = 32;
/// Footer size
const FOOTER_SIZE: usize = 12;

// =============================================================================
// Writer
// =============================================================================

/// Captures FBC packets to a binary datalog file during test execution.
///
/// Usage:
/// ```ignore
/// let mut log = DatalogWriter::create("test_001.fbd", &board_mac, plan_hash)?;
/// // During test — just pass every received packet:
/// log.write_packet(&packet)?;
/// // On test completion:
/// log.finalize()?;
/// ```
pub struct DatalogWriter {
    writer: BufWriter<File>,
    start: Instant,
    record_count: u32,
    body_crc: u32,
}

impl DatalogWriter {
    /// Create a new datalog file and write the header.
    pub fn create(
        path: &str,
        board_mac: &[u8; 6],
        plan_hash: u32,
    ) -> io::Result<Self> {
        let file = File::create(path)?;
        let mut writer = BufWriter::new(file);

        // Write header (32 bytes)
        let mut header = [0u8; HEADER_SIZE];
        header[0..4].copy_from_slice(&MAGIC);
        header[4] = 1; // version
        header[5..11].copy_from_slice(board_mac);

        let epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        header[11..19].copy_from_slice(&epoch_secs.to_le_bytes());
        header[19..23].copy_from_slice(&plan_hash.to_le_bytes());
        // [23..32] reserved, already zero

        writer.write_all(&header)?;

        Ok(Self {
            writer,
            start: Instant::now(),
            record_count: 0,
            body_crc: 0,
        })
    }

    /// Write a single FBC packet as a datalog record.
    ///
    /// This is the hot path — called for every packet during a test.
    /// Just prepends a 4-byte timestamp offset to the raw packet bytes.
    pub fn write_packet(&mut self, packet: &FbcPacket) -> io::Result<()> {
        let offset_ms = self.start.elapsed().as_millis() as u32;
        let raw = packet.serialize();

        // Write: [offset_ms:u32 LE][raw FBC packet (header + payload)]
        let offset_bytes = offset_ms.to_le_bytes();
        self.writer.write_all(&offset_bytes)?;
        self.writer.write_all(&raw)?;

        // Update CRC over the record
        self.body_crc = crc32_update(self.body_crc, &offset_bytes);
        self.body_crc = crc32_update(self.body_crc, &raw);

        self.record_count += 1;
        Ok(())
    }

    /// Finalize the datalog — write footer and flush.
    /// Call this when the test completes or is aborted.
    pub fn finalize(mut self) -> io::Result<u32> {
        // Write footer (12 bytes)
        let mut footer = [0u8; FOOTER_SIZE];
        footer[0..4].copy_from_slice(&self.record_count.to_le_bytes());
        footer[4..8].copy_from_slice(&self.body_crc.to_le_bytes());
        footer[8..12].copy_from_slice(&END_MAGIC);
        self.writer.write_all(&footer)?;
        self.writer.flush()?;

        Ok(self.record_count)
    }

    /// Get current record count (for progress display).
    pub fn record_count(&self) -> u32 {
        self.record_count
    }
}

// =============================================================================
// Reader
// =============================================================================

/// Header parsed from a datalog file.
#[derive(Debug, Clone)]
pub struct DatalogHeader {
    pub version: u8,
    pub board_mac: [u8; 6],
    pub test_start_epoch: u64,
    pub plan_hash: u32,
}

/// A single record from the datalog.
#[derive(Debug, Clone)]
pub struct DatalogRecord {
    /// Milliseconds since test start
    pub offset_ms: u32,
    /// The raw FBC packet
    pub packet: FbcPacket,
}

/// Reads a binary datalog file.
///
/// Usage:
/// ```ignore
/// let reader = DatalogReader::open("test_001.fbd")?;
/// println!("Board: {:?}, Records: {}", reader.header().board_mac, reader.record_count());
/// for record in reader.records()? {
///     let r = record?;
///     println!("{} ms: cmd=0x{:02X} len={}", r.offset_ms, r.packet.header.cmd, r.packet.payload.len());
/// }
/// ```
pub struct DatalogReader {
    header: DatalogHeader,
    record_count: u32,
    body_crc: u32,
    /// Path for re-opening to iterate records
    path: String,
}

impl DatalogReader {
    /// Open and validate a datalog file. Reads header + footer.
    pub fn open(path: &str) -> io::Result<Self> {
        let mut file = File::open(path)?;

        // Read header
        let mut header_buf = [0u8; HEADER_SIZE];
        file.read_exact(&mut header_buf)?;

        if header_buf[0..4] != MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Not an FBC datalog file"));
        }

        let version = header_buf[4];
        let mut board_mac = [0u8; 6];
        board_mac.copy_from_slice(&header_buf[5..11]);
        let test_start_epoch = u64::from_le_bytes(header_buf[11..19].try_into().unwrap());
        let plan_hash = u32::from_le_bytes(header_buf[19..23].try_into().unwrap());

        // Read footer (last 12 bytes)
        file.seek(SeekFrom::End(-(FOOTER_SIZE as i64)))?;
        let mut footer_buf = [0u8; FOOTER_SIZE];
        file.read_exact(&mut footer_buf)?;

        if footer_buf[8..12] != END_MAGIC {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Missing datalog footer (incomplete file?)"));
        }

        let record_count = u32::from_le_bytes(footer_buf[0..4].try_into().unwrap());
        let body_crc = u32::from_le_bytes(footer_buf[4..8].try_into().unwrap());

        Ok(Self {
            header: DatalogHeader { version, board_mac, test_start_epoch, plan_hash },
            record_count,
            body_crc,
            path: path.to_string(),
        })
    }

    pub fn header(&self) -> &DatalogHeader {
        &self.header
    }

    pub fn record_count(&self) -> u32 {
        self.record_count
    }

    /// Iterate all records in the datalog.
    pub fn records(&self) -> io::Result<DatalogRecordIter> {
        let file = File::open(&self.path)?;
        let mut reader = BufReader::new(file);
        // Skip header
        reader.seek(SeekFrom::Start(HEADER_SIZE as u64))?;

        Ok(DatalogRecordIter {
            reader,
            remaining: self.record_count,
        })
    }

    /// Verify body CRC integrity.
    pub fn verify_crc(&self) -> io::Result<bool> {
        let mut crc = 0u32;
        for record_result in self.records()? {
            let record = record_result?;
            let offset_bytes = record.offset_ms.to_le_bytes();
            let raw = record.packet.serialize();
            crc = crc32_update(crc, &offset_bytes);
            crc = crc32_update(crc, &raw);
        }
        Ok(crc == self.body_crc)
    }
}

/// Iterator over datalog records.
pub struct DatalogRecordIter {
    reader: BufReader<File>,
    remaining: u32,
}

impl Iterator for DatalogRecordIter {
    type Item = io::Result<DatalogRecord>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining == 0 {
            return None;
        }

        // Read offset_ms (4 bytes)
        let mut offset_buf = [0u8; 4];
        if let Err(e) = self.reader.read_exact(&mut offset_buf) {
            return Some(Err(e));
        }
        let offset_ms = u32::from_le_bytes(offset_buf);

        // Read FBC header (8 bytes)
        let mut hdr_buf = [0u8; 8];
        if let Err(e) = self.reader.read_exact(&mut hdr_buf) {
            return Some(Err(e));
        }

        let fbc_header = match crate::fbc_protocol::FbcHeader::from_bytes(&hdr_buf) {
            Some(h) => h,
            None => return Some(Err(io::Error::new(
                io::ErrorKind::InvalidData, "Invalid FBC header in datalog record"
            ))),
        };

        // Read payload
        let payload_len = fbc_header.length as usize;
        let mut payload = vec![0u8; payload_len];
        if payload_len > 0 {
            if let Err(e) = self.reader.read_exact(&mut payload) {
                return Some(Err(e));
            }
        }

        self.remaining -= 1;

        Some(Ok(DatalogRecord {
            offset_ms,
            packet: FbcPacket {
                header: fbc_header,
                payload,
            },
        }))
    }
}

// =============================================================================
// CRC helper (simple, no dependency)
// =============================================================================

fn crc32_update(crc: u32, data: &[u8]) -> u32 {
    let mut c = !crc;
    for &byte in data {
        c = CRC32_TABLE[((c ^ byte as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    !c
}

#[rustfmt::skip]
const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fbc_protocol::{FbcPacket, runtime};

    #[test]
    fn test_write_read_roundtrip() {
        let path = "test_datalog_roundtrip.fbd";
        let mac = [0x00, 0x0A, 0x35, 0xC6, 0xB4, 0x2A];

        // Write
        {
            let mut writer = DatalogWriter::create(path, &mac, 0x12345678).unwrap();

            // Simulate 3 heartbeat packets
            for i in 0..3 {
                let mut payload = vec![0u8; 11];
                payload[0..4].copy_from_slice(&(i as u32 * 1000).to_be_bytes()); // cycles
                payload[4..8].copy_from_slice(&0u32.to_be_bytes()); // errors
                payload[8..10].copy_from_slice(&452i16.to_be_bytes()); // temp 45.2C
                payload[10] = 1; // RUNNING

                let pkt = FbcPacket::with_payload(runtime::HEARTBEAT, i, payload);
                writer.write_packet(&pkt).unwrap();
                std::thread::sleep(std::time::Duration::from_millis(10));
            }

            let count = writer.finalize().unwrap();
            assert_eq!(count, 3);
        }

        // Read back
        {
            let reader = DatalogReader::open(path).unwrap();
            assert_eq!(reader.header().version, 1);
            assert_eq!(reader.header().board_mac, mac);
            assert_eq!(reader.header().plan_hash, 0x12345678);
            assert_eq!(reader.record_count(), 3);

            let records: Vec<_> = reader.records().unwrap().collect::<Result<_, _>>().unwrap();
            assert_eq!(records.len(), 3);

            // Verify first record
            assert_eq!(records[0].packet.header.cmd, runtime::HEARTBEAT);
            assert_eq!(records[0].packet.payload.len(), 11);

            // Verify timestamps are monotonic
            assert!(records[1].offset_ms >= records[0].offset_ms);
            assert!(records[2].offset_ms >= records[1].offset_ms);

            // Verify CRC
            assert!(reader.verify_crc().unwrap());
        }

        // Cleanup
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_crc32() {
        let crc = crc32_update(0, b"hello");
        assert_eq!(crc, 0x3610A686);
    }
}
