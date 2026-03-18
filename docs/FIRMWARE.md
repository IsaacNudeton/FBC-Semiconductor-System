# FBC Firmware Architecture

**Last Verified:** March 13, 2026  
**Source:** `firmware/src/*.rs` (verified against actual code)

---

## Overview

Bare-metal Rust firmware for Zynq 7020 ARM Cortex-A9.

**Key characteristics:**
- No OS (`#![no_std]`)
- Boots in <1 second
- Raw Ethernet (EtherType 0x88B5, no TCP/IP)
- 28 FBC protocol commands
- 17 HAL drivers

---

## Boot Sequence

### Entry Point (`main.rs:53`)

```
boot.S (assembly startup)
    │
    ├─ Set stack pointer
    ├─ Clear BSS
    ├─ Enable VFP/NEON
    └─ Jump to main()
        │
        ▼
    main() — PHASE 1: POWER SAFETY
        │
        ├─ Initialize status LED (MIO0)
        ├─ Configure VICOR MIO pins as GPIO via SLCR
        ├─ Set VICOR pins as outputs, disabled
        └─ Toggle LED (boot progress)
        │
        ▼
    PHASE 2: SYSTEM INITIALIZATION
        │
        ├─ Create peripheral handles (FbcCtrl, VectorStatus, etc.)
        ├─ Initialize XADC
        ├─ Check VCCINT/VCCAUX voltages
        └─ Hang with blink code if out of range
        │
        ▼
    PHASE 3: NETWORK INITIALIZATION
        │
        ├─ Enable GEM0 clock
        ├─ Configure GEM0 clocks (TX/RX)
        ├─ Reset PHY
        ├─ Initialize Ethernet (GemEth)
        └─ Send ANNOUNCE packet
        │
        ▼
    MAIN LOOP
        │
        ├─ Poll Ethernet packets
        ├─ Dispatch FBC commands
        ├─ Handle pending requests (VICOR, PMBus, EEPROM, etc.)
        ├─ Send heartbeat (every 100ms)
        └─ Check state transitions
```

---

## Main Loop Architecture (`main.rs:290-500`)

```rust
loop {
    // 1. Poll Ethernet
    if let Some((packet, sender_mac)) = eth.recv_fbc() {
        last_sender_mac = sender_mac;
        if let Some(response) = handler.process(&packet) {
            eth.send_fbc(sender_mac, &response);  // Unicast!
        }
    }

    // 2. Update handler state
    handler.poll();

    // 3. Handle pending requests
    if let Some(log_req) = handler.take_pending_log_read() { /* ... */ }
    if handler.take_pending_log_info() { /* ... */ }
    if handler.take_pending_analog_read() { /* ... */ }
    if let Some(vicor_cmd) = handler.take_pending_vicor() { /* ... */ }
    if let Some(pmbus_cmd) = handler.take_pending_pmbus() { /* ... */ }
    if let Some(eeprom_cmd) = handler.take_pending_eeprom() { /* ... */ }
    if let Some(fastpins_cmd) = handler.take_pending_fastpins() { /* ... */ }
    if let Some(error_log_req) = handler.take_pending_error_log() { /* ... */ }

    // 4. Check state transitions
    if current_state != last_state {
        match current_state {
            ControllerState::Done => { /* test complete */ }
            ControllerState::Error => { send_error_packet() }
            _ => {}
        }
    }

    // 5. Heartbeat (every 100ms)
    if current_state == ControllerState::Running {
        heartbeat_counter += 1;
        if heartbeat_counter >= HEARTBEAT_INTERVAL {
            let heartbeat = handler.build_heartbeat(...);
            eth.send_fbc(last_sender_mac, &heartbeat);
        }
    }
}
```

---

## Interrupt Handling

### IRQ Flow (`main.rs:45-73`, `hal/gic.rs`)

```
FPGA (axi_fbc_ctrl)
    │
    ├─ irq_done (bit 2 of CTRL)
    └─ irq_error (bit 3 of CTRL)
    │
    ▼
GIC (Generic Interrupt Controller)
    │
    ├─ gic_irq_dispatch() in hal/gic.rs
    ├─ Sets IRQ_FLAGS atomically
    └─ Calls _irq_handler in boot.S
    │
    ▼
Firmware
    │
    └─ fbc_irq_handler() — reads status, main loop handles
```

**Note:** Interrupts are enabled via `FbcCtrl::enable_irq()` when START command is received.

---

## HAL Drivers (`firmware/src/hal/`)

### Implemented Drivers (17 total)

| Driver | File | Purpose |
|--------|------|---------|
| **GPIO** | `gpio.rs` | MIO/EMIO GPIO control |
| **SLCR** | `slcr.rs` | System-level control (clocks, resets, MIO mux) |
| **XADC** | `xadc.rs` | XADC monitoring (VCCINT, VCCAUX, temp) |
| **I2C** | `i2c.rs` | I2C master (PMBus, EEPROM) |
| **SPI** | `spi.rs` | SPI master (DAC, ADC) |
| **UART** | `uart.rs` | UART console (debug) |
| **SD** | `sd.rs` | SD card (FAT filesystem) |
| **VICOR** | `vicor.rs` | VICOR core supplies (6 channels) |
| **PMBus** | `pmbus.rs` | PMBus devices (LCPS) |
| **EEPROM** | `eeprom.rs` | EEPROM (BIM, 256 bytes) |
| **MAX11131** | `max11131.rs` | External ADC (16 channels) |
| **BU2505** | `bu2505.rs` | External DAC (10 channels) |
| **DNA** | `dna.rs` | Device DNA (unique ID) |
| **PCAP** | `pcap.rs` | PCAP (FPGA reflash) |
| **Thermal** | `thermal.rs` | Thermal monitoring |
| **DDR** | `ddr.rs` | DDR initialization |
| **GIC** | `gic.rs` | Generic Interrupt Controller |

### Driver Pattern

All HAL drivers follow the same pattern:

```rust
pub struct Peripheral {
    base: usize,  // Memory-mapped base address
}

impl Peripheral {
    pub const fn new() -> Self {
        Self { base: PERIPHERAL_BASE }
    }

    pub fn init(&self) { /* hardware initialization */ }
    pub fn read(&self, reg: u32) -> u32 { /* ... */ }
    pub fn write(&self, reg: u32, val: u32) { /* ... */ }
}
```

**Base addresses** are defined in `firmware/src/regs.rs`.

---

## FBC Protocol Handler (`fbc_protocol.rs`)

### Command Dispatch (`process()` method)

```rust
pub fn process(&mut self, packet: &[u8]) -> Option<FbcPacket> {
    let header = FbcHeader::from_bytes(packet)?;
    let payload = &packet[8..];

    match header.cmd {
        // Setup commands
        setup::ANNOUNCE => self.handle_announce(),
        setup::BIM_STATUS_REQ => self.handle_bim_status_req(payload),
        setup::CONFIGURE => self.handle_configure(payload),
        setup::UPLOAD_VECTORS => self.handle_upload_vectors(payload),

        // Runtime commands
        runtime::START => self.handle_start(),
        runtime::STOP => self.handle_stop(),
        runtime::RESET => self.handle_reset(),
        runtime::STATUS_REQ => self.handle_status_req(),

        // Analog monitoring
        analog::READ_ALL_REQ => self.handle_analog_read(),

        // Power control
        power::VICOR_STATUS_REQ => self.handle_vicor_status_req(),
        power::VICOR_ENABLE => self.handle_vicor_enable(payload),
        power::VICOR_SET_VOLTAGE => self.handle_vicor_set_voltage(payload),
        power::EMERGENCY_STOP => self.handle_emergency_stop(),

        // EEPROM
        eeprom::READ_REQ => self.handle_eeprom_read(payload),
        eeprom::WRITE => self.handle_eeprom_write(payload),

        // Fast pins
        fastpins::READ_REQ => self.handle_fastpins_read(),
        fastpins::WRITE => self.handle_fastpins_write(payload),

        // Error log
        error_log::ERROR_LOG_REQ => self.handle_error_log_req(payload),

        // ... (28 commands total)

        _ => None,
    }
}
```

### Pending Request Pattern

Commands that require slow operations (I2C, SPI, SD) use pending requests:

```rust
// Command handler sets pending flag
fn handle_vicor_status_req(&mut self) -> Option<FbcPacket> {
    self.pending_vicor = Some(PendingVicor::StatusReq);
    None  // No immediate response
}

// Main loop processes pending request
if let Some(vicor_cmd) = handler.take_pending_vicor() {
    match vicor_cmd {
        PendingVicor::StatusReq => {
            let status = vicor.get_status();
            let response = handler.build_vicor_status_response(&status);
            eth.send_fbc(last_sender_mac, &response);
        }
        // ...
    }
}
```

---

## DMA Streaming (`dma.rs`)

### FbcStreamer

```rust
pub struct FbcStreamer {
    dma: AxiDma,
    buffer: DmaBuffer,  // 64KB at 0xFFFC_0000
}

impl FbcStreamer {
    pub fn stream_program(&mut self, fbc_data: &[u8]) -> DmaResult {
        // 1. Align to 32 bytes (256 bits)
        let aligned_len = (fbc_data.len() + 31) & !31;

        // 2. Write to DMA buffer
        let (addr, len) = self.buffer.write(fbc_data)?;

        // 3. Start DMA transfer
        self.dma.send_fbc(addr, len)?;

        // 4. Wait for completion (10M cycles timeout)
        let result = self.dma.wait_mm2s(10_000_000);

        // 5. Mark buffer as consumed
        self.buffer.consume(len);

        result
    }
}
```

**DMA Path:**
```
DDR (0xFFFC_0000)
    │
    ▼
fbc_dma (0x4040_0000)
    │
    │ AXI-Stream (256-bit)
    ▼
axi_stream_fbc
    │
    ▼
fbc_decoder
```

---

## Memory Layout (`link.ld`)

```
MEMORY
{
    DDR (rwx) : ORIGIN = 0x00100000, LENGTH = 128M
    OCM (rwx) : ORIGIN = 0xFFFF0000, LENGTH = 64K
}

SECTIONS
{
    .text : { *(.text*) } > DDR
    .rodata : { *(.rodata*) } > DDR
    .data : { *(.data*) } > DDR
    .bss : { *(.bss*) } > DDR
    _stack_top = 0x10100000;  // Top of DDR
}
```

**Firmware loads at:** 0x00100000 (1MB into DDR)  
**Stack:** Top of DDR (0x10100000)  
**DMA Buffer:** 0xFFFC_0000 (uncached alias)

---

## Error Handling

### Panic Handler

```rust
use panic_halt as _;

// On panic: halt immediately
// No unwinding, no formatting (no_std)
```

### Blink Codes (`hang_with_blink()`)

| Blinks | Error |
|--------|-------|
| 1 | FPGA not responding |
| 2 | VCCINT out of range |
| 3 | VCCAUX out of range |
| 4 | Overtemperature |

---

## Verified Against Code

| Component | File | Lines | Verified |
|-----------|------|-------|----------|
| Boot sequence | `main.rs` | 1-100 | ✅ |
| Main loop | `main.rs` | 290-500 | ✅ |
| Interrupt handler | `main.rs`, `hal/gic.rs` | 45-73 | ✅ |
| HAL drivers | `hal/*.rs` | 17 files | ✅ |
| Protocol handler | `fbc_protocol.rs` | 1000-1500 | ✅ |
| DMA streaming | `dma.rs` | 250-300 | ✅ |
| Memory layout | `link.ld` | All | ✅ |

---

**Related:**
- `docs/PROTOCOL.md` — FBC protocol specification
- `docs/REGISTER_MAP.md` — AXI register map
- `firmware/src/` — Actual source code (authoritative)
