//! Corruption-Resistant SD Flight Recorder
//!
//! Raw sector layout with CRC32 integrity, dual-header redundancy,
//! and boot-time recovery. No filesystem — power loss at any point
//! leaves the log in a recoverable state.
//!
//! ## Sector Layout
//!
//! | Sector | Contents |
//! |--------|----------|
//! | 0      | Header A (primary, CRC32 protected) |
//! | 1      | Header B (backup, CRC32 protected) |
//! | 2-99   | Reserved (future use) |
//! | 100    | First log entry (512 bytes, CRC32 protected) |
//! | 100+N  | Circular buffer wraps at `capacity` entries |
//!
//! ## Write ordering (crash-safe)
//!
//! 1. Write log entry to data sector (with CRC32 in first 4 bytes)
//! 2. Write Header A with updated index
//! 3. Write Header B with updated index
//!
//! If power is lost between steps 1 and 2: header still points to previous
//! valid entry, new entry is orphaned (harmless). Recovery scan finds it.
//!
//! If power is lost between steps 2 and 3: Header A is updated, Header B
//! is stale. Boot picks Header A (higher write_count).

use crate::hal::{SdCard, SdError, crc32};

// =============================================================================
// Constants
// =============================================================================

/// Sector numbers
const HEADER_A_SECTOR: u32 = 0;
const HEADER_B_SECTOR: u32 = 1;
const DATA_START_SECTOR: u32 = 100;

/// Header magic value
const HEADER_MAGIC: u32 = 0xFBC0_DA7A;

/// Log entry magic (first 4 bytes of each entry)
const ENTRY_MAGIC: u16 = 0xFBCE;

/// Maximum entries (sectors 100-9999 = 9900 sectors)
const MAX_CAPACITY: u32 = 9900;

/// Default capacity (matches old layout: ~1000 entries)
const DEFAULT_CAPACITY: u32 = 1000;

// =============================================================================
// Header (stored in sectors 0 and 1)
// =============================================================================

/// Flight Recorder Header — 32 bytes, stored at sector 0 and sector 1.
/// CRC32 covers bytes 0..28 (everything except the CRC field itself).
#[derive(Clone, Copy)]
#[repr(C)]
pub struct FrHeader {
    /// Magic: 0xFBC0_DA7A
    pub magic: u32,
    /// Format version (1)
    pub version: u8,
    /// Padding
    _pad: [u8; 3],
    /// Monotonic write counter (higher = newer)
    pub write_count: u32,
    /// Current write index in circular buffer (0..capacity-1)
    pub write_index: u32,
    /// Total entries written (may exceed capacity due to wrap)
    pub total_entries: u32,
    /// Capacity (number of data sectors)
    pub capacity: u32,
    /// Data start sector
    pub data_start: u32,
    /// CRC32 of bytes 0..28
    pub crc: u32,
}

impl FrHeader {
    pub const SIZE: usize = 32;

    /// Create a fresh header for a newly formatted card
    pub fn new_formatted(capacity: u32) -> Self {
        let mut h = Self {
            magic: HEADER_MAGIC,
            version: 1,
            _pad: [0; 3],
            write_count: 1,
            write_index: 0,
            total_entries: 0,
            capacity,
            data_start: DATA_START_SECTOR,
            crc: 0,
        };
        h.update_crc();
        h
    }

    /// Serialize to 512-byte sector buffer
    pub fn to_sector(&self) -> [u8; 512] {
        let mut buf = [0u8; 512];
        buf[0..4].copy_from_slice(&self.magic.to_le_bytes());
        buf[4] = self.version;
        // buf[5..8] = padding (zeros)
        buf[8..12].copy_from_slice(&self.write_count.to_le_bytes());
        buf[12..16].copy_from_slice(&self.write_index.to_le_bytes());
        buf[16..20].copy_from_slice(&self.total_entries.to_le_bytes());
        buf[20..24].copy_from_slice(&self.capacity.to_le_bytes());
        buf[24..28].copy_from_slice(&self.data_start.to_le_bytes());
        buf[28..32].copy_from_slice(&self.crc.to_le_bytes());
        buf
    }

    /// Deserialize from 512-byte sector buffer
    pub fn from_sector(buf: &[u8; 512]) -> Self {
        Self {
            magic: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            version: buf[4],
            _pad: [0; 3],
            write_count: u32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            write_index: u32::from_le_bytes([buf[12], buf[13], buf[14], buf[15]]),
            total_entries: u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]),
            capacity: u32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]),
            data_start: u32::from_le_bytes([buf[24], buf[25], buf[26], buf[27]]),
            crc: u32::from_le_bytes([buf[28], buf[29], buf[30], buf[31]]),
        }
    }

    /// Recalculate CRC over bytes 0..28
    fn update_crc(&mut self) {
        let buf = self.to_sector();
        self.crc = crc32(&buf[0..28]);
    }

    /// Validate header: magic + CRC
    pub fn is_valid(&self) -> bool {
        if self.magic != HEADER_MAGIC {
            return false;
        }
        if self.version != 1 {
            return false;
        }
        if self.capacity == 0 || self.capacity > MAX_CAPACITY {
            return false;
        }
        // CRC check
        let buf = self.to_sector();
        let computed = crc32(&buf[0..28]);
        computed == self.crc
    }
}

// =============================================================================
// Log Entry (stored in data sectors)
// =============================================================================

/// Log entry header — first 8 bytes of each 512-byte sector
///
/// Layout:
///   [0..2]  entry_magic (0xFBCE)
///   [2..4]  entry_type (heartbeat=1, event=2, boot=3)
///   [4..8]  sequence number (monotonic)
///   [8..12] timestamp_ms (milliseconds since boot)
///   [12..16] CRC32 of bytes 0..508 (everything except last 4 bytes)
///   [16..508] payload (type-specific)
///   [508..512] CRC32 of bytes 0..508
pub struct LogEntry;

impl LogEntry {
    /// Entry types
    pub const TYPE_HEARTBEAT: u16 = 1;
    pub const TYPE_EVENT: u16 = 2;
    pub const TYPE_BOOT: u16 = 3;

    /// CRC offset: last 4 bytes of sector
    const CRC_OFFSET: usize = 508;

    /// Build a log entry sector buffer
    ///
    /// Places entry_magic, type, sequence, timestamp in the header area,
    /// copies payload into bytes [16..16+payload_len], and computes CRC32.
    pub fn build(
        entry_type: u16,
        sequence: u32,
        timestamp_ms: u32,
        payload: &[u8],
    ) -> [u8; 512] {
        let mut buf = [0u8; 512];

        // Entry header
        buf[0..2].copy_from_slice(&ENTRY_MAGIC.to_le_bytes());
        buf[2..4].copy_from_slice(&entry_type.to_le_bytes());
        buf[4..8].copy_from_slice(&sequence.to_le_bytes());
        buf[8..12].copy_from_slice(&timestamp_ms.to_le_bytes());

        // Payload (up to 492 bytes: 512 - 16 header - 4 CRC)
        let max_payload = 492;
        let copy_len = payload.len().min(max_payload);
        buf[16..16 + copy_len].copy_from_slice(&payload[..copy_len]);

        // CRC32 over bytes 0..508
        let entry_crc = crc32(&buf[0..Self::CRC_OFFSET]);
        buf[Self::CRC_OFFSET..512].copy_from_slice(&entry_crc.to_le_bytes());

        buf
    }

    /// Validate a log entry sector: check magic + CRC
    pub fn is_valid(buf: &[u8; 512]) -> bool {
        let magic = u16::from_le_bytes([buf[0], buf[1]]);
        if magic != ENTRY_MAGIC {
            return false;
        }
        let stored_crc = u32::from_le_bytes([
            buf[Self::CRC_OFFSET],
            buf[Self::CRC_OFFSET + 1],
            buf[Self::CRC_OFFSET + 2],
            buf[Self::CRC_OFFSET + 3],
        ]);
        let computed = crc32(&buf[0..Self::CRC_OFFSET]);
        stored_crc == computed
    }

    /// Extract sequence number from a valid entry
    pub fn sequence(buf: &[u8; 512]) -> u32 {
        u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]])
    }

    /// Extract entry type from a valid entry
    pub fn entry_type(buf: &[u8; 512]) -> u16 {
        u16::from_le_bytes([buf[2], buf[3]])
    }
}

// =============================================================================
// SD Health State
// =============================================================================

/// SD card health state (reported in LOG_INFO)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SdHealth {
    /// Card is healthy, headers consistent
    Ok = 0,
    /// Recovered from single-header corruption (auto-repaired at boot)
    Recovered = 1,
    /// Reformatted (both headers were corrupt, fresh start)
    Reformatted = 2,
    /// SD card not present or unreadable
    Missing = 3,
    /// Recovered via scan (both headers corrupt, data recovered)
    ScannedAndRecovered = 4,
}

// =============================================================================
// Flight Recorder
// =============================================================================

/// Corruption-resistant flight recorder using raw SD sectors
pub struct FlightRecorder {
    /// Current header state
    header: FrHeader,
    /// SD health state (from boot recovery)
    pub health: SdHealth,
    /// Global sequence counter (monotonic across power cycles)
    sequence: u32,
    /// Whether SD card is usable
    pub sd_ok: bool,
}

impl FlightRecorder {
    /// Create an uninitialized flight recorder
    pub const fn new() -> Self {
        Self {
            header: FrHeader {
                magic: 0,
                version: 0,
                _pad: [0; 3],
                write_count: 0,
                write_index: 0,
                total_entries: 0,
                capacity: DEFAULT_CAPACITY,
                data_start: DATA_START_SECTOR,
                crc: 0,
            },
            health: SdHealth::Missing,
            sequence: 0,
            sd_ok: false,
        }
    }

    /// Initialize: try to recover existing log, format if necessary.
    ///
    /// Boot recovery sequence:
    /// 1. Try Header A → if valid, use it
    /// 2. Try Header B → if valid, use it, repair Header A
    /// 3. Both corrupt → scan data sectors to find last valid entry
    /// 4. Scan fails → format (fresh start)
    pub fn init(&mut self, sd: &SdCard) -> Result<(), SdError> {
        // Try to read both headers
        let sector_a = sd.read_block(HEADER_A_SECTOR, 1000);
        let sector_b = sd.read_block(HEADER_B_SECTOR, 1000);

        let header_a = sector_a.ok().map(|s| FrHeader::from_sector(&s));
        let header_b = sector_b.ok().map(|s| FrHeader::from_sector(&s));

        let valid_a = header_a.as_ref().map_or(false, |h| h.is_valid());
        let valid_b = header_b.as_ref().map_or(false, |h| h.is_valid());

        match (valid_a, valid_b) {
            (true, true) => {
                // Both valid — pick the one with higher write_count
                let a = header_a.unwrap();
                let b = header_b.unwrap();
                if a.write_count >= b.write_count {
                    self.header = a;
                } else {
                    self.header = b;
                    // Repair A to match B
                    let _ = sd.write_block(HEADER_A_SECTOR, &self.header.to_sector());
                }
                self.health = SdHealth::Ok;
            }
            (true, false) => {
                // A valid, B corrupt — use A, repair B
                self.header = header_a.unwrap();
                let _ = sd.write_block(HEADER_B_SECTOR, &self.header.to_sector());
                self.health = SdHealth::Recovered;
            }
            (false, true) => {
                // B valid, A corrupt — use B, repair A
                self.header = header_b.unwrap();
                let _ = sd.write_block(HEADER_A_SECTOR, &self.header.to_sector());
                self.health = SdHealth::Recovered;
            }
            (false, false) => {
                // Both corrupt — try scan recovery
                if self.scan_recover(sd) {
                    self.health = SdHealth::ScannedAndRecovered;
                } else {
                    // No recoverable data — format fresh
                    self.format(sd)?;
                    self.health = SdHealth::Reformatted;
                }
            }
        }

        // Set sequence counter to continue from where we left off
        self.sequence = self.header.total_entries;

        // Write a boot entry
        let boot_payload = [0u8; 4]; // Minimal boot marker
        self.write_entry(sd, LogEntry::TYPE_BOOT, &boot_payload)?;

        self.sd_ok = true;
        Ok(())
    }

    /// Scan data sectors to find the highest valid sequence number.
    /// Rebuilds header from scan results. Returns true if any data recovered.
    fn scan_recover(&mut self, sd: &SdCard) -> bool {
        let capacity = DEFAULT_CAPACITY;
        let mut max_seq: u32 = 0;
        let mut max_seq_index: u32 = 0;
        let mut found_any = false;

        // Scan all data sectors for valid entries
        for i in 0..capacity {
            let sector = DATA_START_SECTOR + i;
            if let Ok(buf) = sd.read_block(sector, 1000) {
                if LogEntry::is_valid(&buf) {
                    let seq = LogEntry::sequence(&buf);
                    if !found_any || seq > max_seq {
                        max_seq = seq;
                        max_seq_index = i;
                        found_any = true;
                    }
                }
            }
        }

        if !found_any {
            return false;
        }

        // Rebuild header from scan results
        // write_index points to NEXT slot (one after the highest sequence)
        self.header = FrHeader {
            magic: HEADER_MAGIC,
            version: 1,
            _pad: [0; 3],
            write_count: 1,
            write_index: (max_seq_index + 1) % capacity,
            total_entries: max_seq + 1,
            capacity,
            data_start: DATA_START_SECTOR,
            crc: 0,
        };
        // CRC
        let buf = self.header.to_sector();
        self.header.crc = crc32(&buf[0..28]);

        // Write recovered headers
        let sector_buf = self.header.to_sector();
        let _ = sd.write_block(HEADER_A_SECTOR, &sector_buf);
        let _ = sd.write_block(HEADER_B_SECTOR, &sector_buf);

        true
    }

    /// Format the SD card for flight recording (destructive — erases all log data)
    pub fn format(&mut self, sd: &SdCard) -> Result<(), SdError> {
        self.header = FrHeader::new_formatted(DEFAULT_CAPACITY);
        self.sequence = 0;

        // Write both headers
        let sector_buf = self.header.to_sector();
        sd.write_block(HEADER_A_SECTOR, &sector_buf)?;
        sd.write_block(HEADER_B_SECTOR, &sector_buf)?;

        self.health = SdHealth::Ok;
        self.sd_ok = true;
        Ok(())
    }

    /// Attempt to repair a potentially corrupted SD card.
    ///
    /// Strategy:
    /// 1. Read both headers, check validity
    /// 2. If one is valid, repair the other
    /// 3. If both corrupt, scan data sectors
    /// 4. If scan finds data, rebuild headers
    /// 5. If no data, format fresh
    ///
    /// Returns the health state after repair.
    pub fn repair(&mut self, sd: &SdCard) -> SdHealth {
        // Re-run the full init sequence (it already does repair)
        match self.init(sd) {
            Ok(()) => self.health,
            Err(_) => {
                self.health = SdHealth::Missing;
                self.sd_ok = false;
                SdHealth::Missing
            }
        }
    }

    /// Write a log entry (heartbeat, event, or boot marker)
    ///
    /// Crash-safe write order:
    /// 1. Write data sector (entry with CRC)
    /// 2. Update Header A (with new index + write_count)
    /// 3. Update Header B (backup)
    pub fn write_entry(
        &mut self,
        sd: &SdCard,
        entry_type: u16,
        payload: &[u8],
    ) -> Result<(), SdError> {
        if !self.sd_ok {
            return Err(SdError::CardNotPresent);
        }

        let timestamp_ms = crate::hal::get_millis() as u32;

        // Step 1: Build and write data sector
        let entry_buf = LogEntry::build(entry_type, self.sequence, timestamp_ms, payload);
        let data_sector = self.header.data_start + self.header.write_index;
        sd.write_block(data_sector, &entry_buf)?;

        // Step 2: Update header state
        self.sequence += 1;
        self.header.write_index = (self.header.write_index + 1) % self.header.capacity;
        self.header.total_entries += 1;
        self.header.write_count += 1;
        self.header.update_crc();

        // Step 3: Write Header A (primary)
        let header_buf = self.header.to_sector();
        let _ = sd.write_block(HEADER_A_SECTOR, &header_buf);

        // Step 4: Write Header B (backup)
        let _ = sd.write_block(HEADER_B_SECTOR, &header_buf);

        Ok(())
    }

    /// Write a heartbeat entry. Payload = raw heartbeat packet bytes.
    pub fn write_heartbeat(&mut self, sd: &SdCard, heartbeat_data: &[u8]) -> Result<(), SdError> {
        self.write_entry(sd, LogEntry::TYPE_HEARTBEAT, heartbeat_data)
    }

    /// Read a specific data sector by index (0..capacity-1)
    pub fn read_entry(&self, sd: &SdCard, index: u32) -> Result<[u8; 512], SdError> {
        if index >= self.header.capacity {
            return Err(SdError::ReadError);
        }
        let sector = self.header.data_start + index;
        sd.read_block(sector, 1000)
    }

    /// Read a raw sector (for protocol compatibility with LOG_READ_REQ)
    pub fn read_sector(&self, sd: &SdCard, sector: u32) -> Result<[u8; 512], SdError> {
        sd.read_block(sector, 1000)
    }

    // =========================================================================
    // Info accessors
    // =========================================================================

    /// Current write index in circular buffer
    pub fn write_index(&self) -> u32 {
        self.header.write_index
    }

    /// Total entries written (may exceed capacity due to wrapping)
    pub fn total_entries(&self) -> u32 {
        self.header.total_entries
    }

    /// Capacity (number of data sectors)
    pub fn capacity(&self) -> u32 {
        self.header.capacity
    }

    /// Data start sector
    pub fn data_start(&self) -> u32 {
        self.header.data_start
    }

    /// Sequence counter (for protocol responses)
    pub fn sequence(&self) -> u32 {
        self.sequence
    }
}
