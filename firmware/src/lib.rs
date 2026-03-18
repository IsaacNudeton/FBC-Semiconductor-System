//! FBC Semiconductor System - Firmware Library
//!
//! Bare metal firmware for Zynq 7020 running FBC hardware acceleration.
//!
//! # Architecture
//!
//! ```text
//! +------------------+     +------------------+
//! | Host PC / GUI    |     | Zynq 7020        |
//! |  - FBC GUI       | Raw |  - ARM Cortex-A9 |
//! |  - FBC Compiler  | Eth |  - FBC Decoder   |
//! +------------------+---->|  - Vector Engine |
//!                          +------------------+
//!                                   |
//!                   +---------------+---------------+
//!                   |                               |
//!                   v                               v
//!          +------------------+            +------------------+
//!          | BIM Pins (128)   |            | Fast Pins (32)   |
//!          | gpio[0:127]      |            | gpio[128:159]    |
//!          | 2-cycle latency  |            | 1-cycle latency  |
//!          +--------+---------+            +--------+---------+
//!                   |                               |
//!                   v                               |
//!          +------------------+                     |
//!          | Quad Board / DUT |<--------------------+
//!          +------------------+
//! ```
//!
//! # Pin Architecture (160 total)
//! - gpio[0:127]: BIM pins through Quad Board (standard test vectors)
//! - gpio[128:159]: Fast pins direct to FPGA (triggers, clocks, handshake)
//!
//! # Modules
//!
//! - `hal` - Hardware Abstraction Layer (Zynq PS peripherals)
//! - `regs` - FPGA register definitions and access
//! - `fbc` - FBC instruction encoding
//! - `dma` - AXI DMA driver for FBC streaming
//! - `net` - Raw Ethernet networking (Zynq GEM, EtherType 0x88B5)
//! - `fbc_protocol` - FBC raw Ethernet protocol (replaces TCP)
//! - `analog` - Unified 32-channel analog monitor (XADC + MAX11131)

#![no_std]
#![allow(dead_code)] // HAL defines all registers, not all used yet

pub mod hal;
pub mod regs;
pub mod fbc;
pub mod dma;
pub mod net;
pub mod fbc_protocol;
pub mod analog;
pub mod fbc_loader;
pub mod fbc_decompress;

// Re-export HAL types
pub use hal::{
    Slcr, I2c, I2cError, Spi, SpiMode, SpiError, Gpio, MioPin,
    Xadc, Uart, Pcap, PmbusDevice, PmbusError,
    PowerSupplyManager, PowerSupplyInfo, MAX_POWER_SUPPLIES,
    lcps_channel_to_addr, lcps_addr_to_channel,
    Bu2505, Max11131, VicorController, VicorError,
    SdCard, SdError, delay_us, delay_ms,
    Eeprom, EepromError, BimEeprom, BimType, RailConfig, EEPROM_SIZE, EEPROM_ADDR,
    Gic, IRQ_FLAGS, IRQ_FLAG_FBC,
};

// Re-export analog monitor (application layer)
pub use analog::{AnalogMonitor, Reading, MonitorError};

// Re-export commonly used items
pub use regs::{
    FbcCtrl, PinCtrl, VectorStatus, FreqCounter, PinType, ClkCtrl, VecClockFreq,
    ErrorBram,
    BIM_PIN_COUNT, FAST_PIN_COUNT, TOTAL_PIN_COUNT,
    BANK35_START, BANK35_END, BANK35_COUNT,
    FAST_SCOPE_TRIG, FAST_ERROR_STROBE, FAST_SYNC_N, FAST_SYNC_P, FAST_SYSCLK_N, FAST_SYSCLK_P,
};
pub use fbc::{FbcOpcode, FbcInstr};
pub use dma::{AxiDma, FbcStreamer, DmaResult};
pub use fbc_loader::{FbcLoader, FbcHeader, LoaderError, parse_header, get_clock_freq};
pub use fbc_decompress::{FbcDecompressor, decompress_to_bytecode, VECTOR_BYTES, MAX_BYTECODE_SIZE};
pub use net::{NetConfig, GemEth};

// Re-export FBC Protocol (raw Ethernet) - primary protocol
pub use fbc_protocol::{
    FbcProtocolHandler, FbcPacket, FbcHeader as FbcProtoHeader, ControllerState,
    AnnouncePayload, HeartbeatPayload, StatusPayload,
    ConfigPayload, ConfigResult, TelemetryData,
    PendingVicor, PendingEeprom, PendingFastPins, PendingErrorLog, ErrorLogEntry,
    ETHERTYPE_FBC, FBC_MAGIC, MAX_PAYLOAD,
    setup, runtime, error_log,
};

