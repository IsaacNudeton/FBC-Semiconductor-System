//! DDR Double-Buffer + SD Pattern Storage
//!
//! Architecture (replaces fixed 8-slot model):
//!   - SD card holds ALL patterns for the device (hundreds of .fbc files)
//!   - DDR has two regions: ACTIVE (FPGA reads) and STAGING (next pattern loads)
//!   - Plan executor swaps regions on step transition
//!   - Board is fully autonomous — no PC needed during 500-hour test
//!
//! DDR Layout (512MB total):
//!   0x0010_0000 - 0x002F_FFFF  Firmware (2MB)
//!   0x0030_0000 - 0x0030_0FFF  Metadata (4KB — checkpoint, pattern table cache)
//!   0x0030_1000 - 0x0030_1FFF  Plan checkpoint (persists across warm reset)
//!   0x0040_0000 - 0x0FFF_FFFF  DDR Region A (252MB)
//!   0x1000_0000 - 0x1FFF_FFFF  DDR Region B (256MB)
//!
//! SD Layout (sectors):
//!   0-7:         SD header (magic, version, pattern count, plan)
//!   8-2047:      Pattern directory (256 entries × 8 sectors = index)
//!   2048-4095:   Flight recorder (existing)
//!   4096+:       Pattern data (sequential .fbc files)
//!
//! Flow:
//!   Boot → read EEPROM project_code → find SD header → load plan
//!   Run  → load pattern[step.pattern_id] from SD → DDR staging → swap → execute
//!   Loop → staging pre-loaded with next pattern during current execution

// DDR double-buffer: direct pointer writes for region loading

// =============================================================================
// DDR Memory Layout
// =============================================================================

/// DDR Region A start
const REGION_A_BASE: usize = 0x0040_0000;
/// DDR Region B start
const REGION_B_BASE: usize = 0x1000_0000;
/// Maximum pattern size per region (~252MB for A, ~256MB for B)
pub const REGION_A_SIZE: usize = 0x0FC0_0000; // 252MB
pub const REGION_B_SIZE: usize = 0x1000_0000; // 256MB

// =============================================================================
// SD Layout Constants
// =============================================================================

/// SD header sector
pub const SD_HEADER_SECTOR: u32 = 0;
/// SD header magic
const SD_HEADER_MAGIC: u32 = 0x4642_5344; // "FBSD"
/// SD pattern directory starts here
pub const SD_DIRECTORY_SECTOR: u32 = 8;
/// SD pattern data starts here (after flight recorder at 2048-4095)
pub const SD_PATTERN_DATA_SECTOR: u32 = 4096;
/// Maximum patterns on SD
pub const MAX_PATTERNS: usize = 256;

// =============================================================================
// SD Header (sector 0, 512 bytes)
// =============================================================================

/// SD card header — identifies what's stored
#[derive(Clone, Copy)]
pub struct SdHeader {
    /// Magic: 0x46425344 ("FBSD")
    pub magic: u32,
    /// Format version
    pub version: u8,
    /// Number of patterns stored
    pub pattern_count: u16,
    /// BIM project code (from EEPROM — ties SD content to device type)
    pub project_code: u8,
    /// BIM serial (invalidation key)
    pub bim_serial: u32,
    /// Total data sectors used (for free space calculation)
    pub total_data_sectors: u32,
}

impl SdHeader {
    pub fn from_bytes(data: &[u8; 512]) -> Option<Self> {
        let magic = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        if magic != SD_HEADER_MAGIC {
            return None;
        }
        Some(Self {
            magic,
            version: data[4],
            pattern_count: u16::from_le_bytes([data[5], data[6]]),
            project_code: data[7],
            bim_serial: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            total_data_sectors: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
        })
    }

    pub fn to_bytes(&self) -> [u8; 512] {
        let mut buf = [0u8; 512];
        buf[0..4].copy_from_slice(&self.magic.to_le_bytes());
        buf[4] = self.version;
        buf[5..7].copy_from_slice(&self.pattern_count.to_le_bytes());
        buf[7] = self.project_code;
        buf[8..12].copy_from_slice(&self.bim_serial.to_le_bytes());
        buf[12..16].copy_from_slice(&self.total_data_sectors.to_le_bytes());
        buf
    }
}

// =============================================================================
// Pattern Directory Entry (16 bytes each, 32 per sector, 256 max)
// =============================================================================

/// Where a pattern lives on SD
#[derive(Clone, Copy)]
pub struct PatternEntry {
    /// Starting sector on SD (relative to SD_PATTERN_DATA_SECTOR)
    pub start_sector: u32,
    /// Size in bytes
    pub size_bytes: u32,
    /// Number of uncompressed vectors (from .fbc header)
    pub num_vectors: u32,
    /// Vector clock Hz (from .fbc header)
    pub vec_clock_hz: u32,
}

impl PatternEntry {
    pub const EMPTY: Self = Self {
        start_sector: 0,
        size_bytes: 0,
        num_vectors: 0,
        vec_clock_hz: 0,
    };

    pub fn is_valid(&self) -> bool {
        self.size_bytes > 0
    }

    /// Number of 512-byte sectors this pattern occupies
    pub fn sector_count(&self) -> u32 {
        (self.size_bytes + 511) / 512
    }

    pub fn from_bytes(data: &[u8]) -> Self {
        if data.len() < 16 {
            return Self::EMPTY;
        }
        Self {
            start_sector: u32::from_le_bytes([data[0], data[1], data[2], data[3]]),
            size_bytes: u32::from_le_bytes([data[4], data[5], data[6], data[7]]),
            num_vectors: u32::from_le_bytes([data[8], data[9], data[10], data[11]]),
            vec_clock_hz: u32::from_le_bytes([data[12], data[13], data[14], data[15]]),
        }
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buf = [0u8; 16];
        buf[0..4].copy_from_slice(&self.start_sector.to_le_bytes());
        buf[4..8].copy_from_slice(&self.size_bytes.to_le_bytes());
        buf[8..12].copy_from_slice(&self.num_vectors.to_le_bytes());
        buf[12..16].copy_from_slice(&self.vec_clock_hz.to_le_bytes());
        buf
    }
}

// =============================================================================
// DDR Double-Buffer Manager
// =============================================================================

/// Which DDR region is active (FPGA reads from) vs staging (next pattern loads to)
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActiveRegion {
    A,
    B,
}

impl ActiveRegion {
    pub fn other(self) -> Self {
        match self {
            ActiveRegion::A => ActiveRegion::B,
            ActiveRegion::B => ActiveRegion::A,
        }
    }
}

/// DDR double-buffer manager
pub struct DdrBuffer {
    /// Which region the FPGA is currently reading from
    active: ActiveRegion,
    /// Pattern loaded in region A (index, or None)
    pattern_a: Option<u16>,
    /// Pattern loaded in region B (index, or None)
    pattern_b: Option<u16>,
    /// Size of data in region A
    size_a: u32,
    /// Size of data in region B
    size_b: u32,
    /// Whether staging region has been loaded and is ready to swap
    staging_ready: bool,
}

impl DdrBuffer {
    pub const fn new() -> Self {
        Self {
            active: ActiveRegion::A,
            pattern_a: None,
            pattern_b: None,
            size_a: 0,
            size_b: 0,
            staging_ready: false,
        }
    }

    /// Get the DDR address + size for the active region (what FPGA reads)
    pub fn active_region(&self) -> (usize, u32) {
        match self.active {
            ActiveRegion::A => (REGION_A_BASE, self.size_a),
            ActiveRegion::B => (REGION_B_BASE, self.size_b),
        }
    }

    /// Get the DDR address for the staging region (where next pattern loads)
    pub fn staging_addr(&self) -> usize {
        match self.active {
            ActiveRegion::A => REGION_B_BASE, // staging is B
            ActiveRegion::B => REGION_A_BASE, // staging is A
        }
    }

    /// Get max size for staging region
    pub fn staging_max_size(&self) -> usize {
        match self.active {
            ActiveRegion::A => REGION_B_SIZE,
            ActiveRegion::B => REGION_A_SIZE,
        }
    }

    /// Mark staging region as loaded with a pattern
    pub fn set_staging_loaded(&mut self, pattern_id: u16, size: u32) {
        match self.active {
            ActiveRegion::A => { self.pattern_b = Some(pattern_id); self.size_b = size; }
            ActiveRegion::B => { self.pattern_a = Some(pattern_id); self.size_a = size; }
        }
        self.staging_ready = true;
    }

    /// Swap active and staging regions. Returns new active DDR address + size.
    pub fn swap(&mut self) -> (usize, u32) {
        self.active = self.active.other();
        self.staging_ready = false;
        self.active_region()
    }

    /// Load a pattern directly into region A (first load, no swap needed)
    pub fn set_initial_load(&mut self, pattern_id: u16, size: u32) {
        self.active = ActiveRegion::A;
        self.pattern_a = Some(pattern_id);
        self.size_a = size;
        self.staging_ready = false;
    }

    /// Is the staging region ready for a swap?
    pub fn is_staging_ready(&self) -> bool {
        self.staging_ready
    }

    /// Current active pattern ID
    pub fn active_pattern(&self) -> Option<u16> {
        match self.active {
            ActiveRegion::A => self.pattern_a,
            ActiveRegion::B => self.pattern_b,
        }
    }

    /// Begin a chunked SD → DDR load. Call `load_chunk()` repeatedly from
    /// the main loop until it returns `LoadProgress::Done`. Between calls,
    /// the safety monitor, heartbeat, and Ethernet handler all run normally.
    ///
    /// Each `load_chunk()` reads CHUNK_SECTORS (64 = 32KB) which takes ~1.3ms
    /// at 25MB/s SDIO. Safety loop runs at 500ms interval — zero missed checks.
    pub fn begin_load(
        &mut self,
        entry: &PatternEntry,
        pattern_id: u16,
    ) -> Result<SdLoadState, SdLoadError> {
        let max_size = self.staging_max_size();
        if entry.size_bytes as usize > max_size {
            return Err(SdLoadError::PatternTooLarge);
        }
        if !entry.is_valid() {
            return Err(SdLoadError::InvalidEntry);
        }

        Ok(SdLoadState {
            ddr_addr: self.staging_addr(),
            sd_start: SD_PATTERN_DATA_SECTOR + entry.start_sector,
            total_sectors: entry.sector_count(),
            sectors_loaded: 0,
            pattern_id,
            total_bytes: entry.size_bytes,
        })
    }

    /// Load the next chunk of sectors from SD into DDR.
    /// Returns Done when complete, InProgress otherwise.
    /// Call this once per main loop iteration — safety checks run between calls.
    pub fn load_chunk(
        &mut self,
        sd: &crate::hal::SdCard,
        state: &mut SdLoadState,
    ) -> Result<LoadProgress, SdLoadError> {
        /// Sectors per chunk — 64 sectors × 512B = 32KB per call (~1.3ms at 25MB/s)
        const CHUNK_SECTORS: u32 = 64;

        let remaining = state.total_sectors - state.sectors_loaded;
        let to_read = remaining.min(CHUNK_SECTORS);

        for i in 0..to_read {
            let sector = state.sd_start + state.sectors_loaded + i;
            let block = sd.read_block(sector, 1000)
                .map_err(|_| SdLoadError::ReadError)?;

            let offset = (state.sectors_loaded + i) as usize * 512;
            let dst = (state.ddr_addr + offset) as *mut u8;
            unsafe {
                core::ptr::copy_nonoverlapping(block.as_ptr(), dst, 512);
            }
        }

        state.sectors_loaded += to_read;

        if state.sectors_loaded >= state.total_sectors {
            self.set_staging_loaded(state.pattern_id, state.total_bytes);
            Ok(LoadProgress::Done)
        } else {
            Ok(LoadProgress::InProgress {
                loaded: state.sectors_loaded,
                total: state.total_sectors,
            })
        }
    }

    /// Blocking convenience: load entire pattern (for initial load at boot).
    /// Only use when safety monitor hasn't started yet.
    pub fn load_initial_from_sd(
        &mut self,
        sd: &crate::hal::SdCard,
        entry: &PatternEntry,
        pattern_id: u16,
    ) -> Result<u32, SdLoadError> {
        let mut state = self.begin_load(entry, pattern_id)?;
        loop {
            match self.load_chunk(sd, &mut state)? {
                LoadProgress::Done => {
                    self.active = ActiveRegion::A;
                    self.pattern_a = Some(pattern_id);
                    self.size_a = entry.size_bytes;
                    self.staging_ready = false;
                    return Ok(entry.size_bytes);
                }
                LoadProgress::InProgress { .. } => continue,
            }
        }
    }

    // Old blocking load_initial_from_sd removed — replaced by chunked version above
}

// =============================================================================
// Pattern Directory (cached from SD)
// =============================================================================

/// Cached pattern directory — loaded from SD at boot
pub struct PatternDirectory {
    pub entries: [PatternEntry; MAX_PATTERNS],
    pub count: u16,
}

impl PatternDirectory {
    pub const fn new() -> Self {
        Self {
            entries: [PatternEntry::EMPTY; MAX_PATTERNS],
            count: 0,
        }
    }

    /// Load directory from SD card (sectors 8-15, 32 entries per sector)
    pub fn load_from_sd(&mut self, sd: &crate::hal::SdCard) -> Result<u16, SdLoadError> {
        self.count = 0;
        let sectors_needed = (MAX_PATTERNS * 16 + 511) / 512; // 8 sectors for 256 entries

        for sec in 0..sectors_needed as u32 {
            let block = sd.read_block(SD_DIRECTORY_SECTOR + sec, 1000)
                .map_err(|_| SdLoadError::ReadError)?;

            // 32 entries per 512-byte sector (16 bytes each)
            for i in 0..32 {
                let idx = (sec as usize * 32) + i;
                if idx >= MAX_PATTERNS { break; }

                let off = i * 16;
                let entry = PatternEntry::from_bytes(&block[off..off + 16]);
                self.entries[idx] = entry;
                if entry.is_valid() && idx as u16 >= self.count {
                    self.count = idx as u16 + 1;
                }
            }
        }

        Ok(self.count)
    }

    /// Get a pattern entry by index
    pub fn get(&self, index: u16) -> Option<&PatternEntry> {
        if (index as usize) < MAX_PATTERNS && self.entries[index as usize].is_valid() {
            Some(&self.entries[index as usize])
        } else {
            None
        }
    }
}

// =============================================================================
// Errors
// =============================================================================

/// In-progress SD → DDR load state (held by main loop between chunks)
pub struct SdLoadState {
    pub ddr_addr: usize,
    pub sd_start: u32,
    pub total_sectors: u32,
    pub sectors_loaded: u32,
    pub pattern_id: u16,
    pub total_bytes: u32,
}

/// Result of a load_chunk() call
#[derive(Debug, Clone, Copy)]
pub enum LoadProgress {
    /// More chunks needed
    InProgress { loaded: u32, total: u32 },
    /// All sectors loaded, staging region ready
    Done,
}

#[derive(Debug, Clone, Copy)]
pub enum SdLoadError {
    PatternTooLarge,
    InvalidEntry,
    ReadError,
    NotFound,
}

// =============================================================================
// Legacy compatibility — keep MAX_SLOTS for existing code
// =============================================================================

/// Legacy: MAX_SLOTS used by testplan.rs for validation
pub const MAX_SLOTS: usize = MAX_PATTERNS;
