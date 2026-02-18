# FBC Semiconductor System - Project Status

**Last Updated:** February 2026

## What This Is

A burn-in test system for semiconductor chips using ~44 Zynq 7020 FPGA boards per system. Replaces the 2016 Sonoma/kzhang_v2 Linux-based system with bare-metal Rust firmware and a custom FPGA toolchain.

---

## Component Status

| Component | Status | Notes |
|-----------|--------|-------|
| **RTL Design** | 95% | 160-pin I/O, 4 clock outputs, all AXI peripherals |
| **Firmware** | 95% | GEM driver, FBC protocol, all HAL drivers complete |
| **FPGA Toolchain** | 99% | Separate repo, ONETWO bitstream complete |
| **GUI (Frontend)** | 95% | All panels implemented |
| **GUI (Backend)** | 70% | Tauri commands need wiring to FBC client |
| **Hardware Testing** | 0% | **CRITICAL BLOCKER** |

---

## What's Actually Done

### Firmware Network Stack (COMPLETE)
Located in `firmware/src/net.rs` - fully implemented:
- **GEM driver** - Zynq GigE MAC with buffer descriptors
- **PHY initialization** - Auto-negotiation, RGMII delays for KSZ9021/KSZ9031/Marvell
- **FBC protocol** - Raw Ethernet (EtherType 0x88B5), send/recv methods
- **UDP support** - Packet building with checksums
- **ARP handling** - Responds to ARP requests

### Firmware Main Loop (COMPLETE)
Located in `firmware/src/main.rs`:
- **Boot sequence** - Power safety, XADC checks, peripheral init
- **Network init** - GEM0 enabled, PHY reset, config from DNA
- **ANNOUNCE on boot** - Broadcasts identity to network
- **Command processing** - `handler.process(&packet)` dispatches all commands
- **Heartbeat** - Sent every 100ms during Running state
- **Flight Recorder** - Logs to SD card sectors 1001-2000
- **State machine** - Idle/Running/Done/Error transitions

### Firmware Protocol Handler (COMPLETE)
Located in `firmware/src/fbc_protocol.rs`:
- All FBC packet types defined
- STATUS_REQ/RSP, HEARTBEAT, ERROR
- VICOR commands (enable, set voltage, sequence)
- PMBus commands (enable/disable by address)
- EEPROM read/write
- Fast pin control (read/write)
- Analog monitor (32 channels)
- Flight recorder access

---

## What's Actually Blocking

### 1. Hardware Testing (CRITICAL)
**Everything is implemented, but nothing is validated on real hardware.**

Need:
- Zynq 7020 board (Sonoma controller)
- JTAG cable or SD card for flashing
- Network connection

Testing sequence:
1. Flash bitstream (JTAG or SD boot)
2. Boot firmware, watch UART output
3. Verify ANNOUNCE packet on network
4. Send STATUS_REQ, verify response
5. Test AXI register access
6. Run simple vector, verify GPIO toggling

### 2. GUI Backend (Medium)
Frontend panels exist but need to send actual FBC packets.
- Wire Tauri commands to FBC client library
- Handle async responses (STATUS_RSP, HEARTBEAT)
- Display live telemetry

---

## Files to Know

| Purpose | File | Status |
|---------|------|--------|
| GEM Ethernet | `firmware/src/net.rs` | Complete |
| FBC Protocol | `firmware/src/fbc_protocol.rs` | Complete |
| Main Loop | `firmware/src/main.rs` | Complete |
| Top-level RTL | `rtl/system_top.v` | Complete |
| FBC Core | `rtl/fbc_top.v` | Complete |
| GUI Backend | `gui/src-tauri/src/lib.rs` | 70% |

---

## Build Commands

```bash
# Build firmware
cd firmware && cargo build --release --target armv7a-none-eabi

# Build GUI
cd gui && npm run tauri build

# Build toolchain (separate repo)
cd C:\Dev\projects\fpga-toolchain && cargo build --release
```

---

## What Would "Final Product" Look Like

1. **Flash SD card** with BOOT.BIN (FSBL + bitstream + firmware)
2. **Insert into controller**, power on
3. **Controller boots** in <1 second, sends ANNOUNCE
4. **GUI connects**, displays board status
5. **Load test vectors**, configure power supplies
6. **Run test**, monitor in real-time
7. **View results**, export CSV/STDF

Current gap: Step 1-4 not validated on hardware.
