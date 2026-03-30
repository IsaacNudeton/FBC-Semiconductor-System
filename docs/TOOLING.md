# FBC Semiconductor System — Complete Tooling Inventory

Last updated: March 19, 2026

Everything installed on this machine that touches the FBC project.
Organized by: what it does, where it lives, what state it's in.

---

## 1. Language Toolchains

### Rust (Primary — firmware + host CLI + GUI backend)

| Component | Version | Path |
|-----------|---------|------|
| rustc | 1.94.0 (2026-03-02) | scoop install |
| cargo | 1.94.0 | with rustc |
| Target: x86_64-pc-windows-msvc | installed | Host CLI, GUI backend |
| Target: armv7a-none-eabi | installed | Bare-metal ARM firmware |

**Builds:**
- `firmware/` → bare-metal ARM ELF (armv7a-none-eabi), 453 KB
- `host/` → CLI binary + shared library (x86_64), raw Ethernet FBC client
- `app/` → **Native wgpu GUI** (x86_64), 7.4MB release binary, 8 deps, DX12
- `gui/src-tauri/` → Legacy Tauri backend (superseded by `app/`)

### C/C++ (Pattern converter C engine + WinEth driver)

| Component | Version | Path |
|-----------|---------|------|
| GCC (MinGW) | 13.2.0 (x86_64-posix-seh) | `C:\Dev\tools\mingw64\bin\` |
| Clang/LLVM | 21.1.8 | `C:\Program Files\LLVM\bin\` |
| .NET | 9.0.312 | `C:\Program Files\dotnet\` |

**Used for:**
- `gui/src-tauri/c-engine/` — pattern converter (gen_hex, gen_seq, gen_fbc, LRM schema)
- `C:\Dev\tools\wineth\` — custom NDIS filter driver for raw Ethernet

### Python (JTAG tools + scripting)

| Component | Version | Path |
|-----------|---------|------|
| Python | 3.13.3 | `C:\Users\isaac\AppData\Local\Programs\Python\Python313\` |
| pip | 25.2 | with Python |

**Used for:**
- `C:\Dev\xyzt_pico_Hardware\fpga_jtag.py` — FPGA bitstream programming via FT232H
- `C:\Dev\xyzt_pico_Hardware\arm_loader.py` — ARM firmware loading via JTAG DAP

### JavaScript/TypeScript (GUI frontend)

| Component | Version | Path |
|-----------|---------|------|
| Node.js | 24.10.0 | scoop install |
| npm | 11.6.1 | with Node |
| pnpm | 10.30.3 | `C:\.pnpm-store\v10` (store) |

**Used for:**
- `gui/src/` — React + Three.js frontend (Tauri app)

---

## 2. FPGA Tools

### Xilinx Vivado 2023.1 (THE standard tool)

| Path | `C:\Xilinx\Vivado\2023.1\` |
|------|---|
| **vivado.bat** | Full IDE + batch synthesis/implementation/bitgen |
| **xsdb.bat** | JTAG debug shell — program PL, load PS firmware, read/write memory |
| **hw_server.bat** | Hardware server — connects to JTAG cables (Digilent, Platform Cable, FTDI?) |
| **bootgen.bat** | Generates BOOT.BIN from BIF file (FSBL + bitstream + app) |
| **program_ftdi.bat** | Reprograms FTDI EEPROM for Xilinx cable compatibility |
| **xvlog/xvhdl/xelab/xsim.bat** | Verilog/VHDL simulation toolchain |
| **loader.bat** | Flash programmer |
| **updatemem.bat** | Inject ELF into bitstream |
| **svf_utility.bat** | SVF/XSVF JTAG file generation |
| **cs_server.bat** | Chipscope server |

**NOT installed:** Vitis (no xsct, no hsi, no platform project generation)

**What we've done with Vivado:**
- `scripts/build_bitstream.tcl` — full synth+impl+bitgen, produces `build/fbc_system.bit` (3.9 MB)
- PS7 IP auto-generated (Zynq PS config: clocks, DDR, MIO, AXI)
- Build reports in `build/` (utilization, timing, power, IO, DRC, clocks)

**What we HAVEN'T done with Vivado:**
- No `.xsa` exported (hardware description for software tools)
- Never used xsdb for JTAG (used Python scripts instead)
- Never used hw_server (used direct FTDI MPSSE instead)
- Never used bootgen (hand-loaded via JTAG)
- Never used xsim (testbenches exist in `tb/` but untested in Vivado sim)

### Custom FPGA Toolchain (experimental, NOT used for production)

| Path | `C:\Dev\projects\fpga-toolchain\` |
|------|---|
| **src/** | 33 Rust modules — synth, place, route, bitstream gen |
| **scripts/** | vivado_all.tcl, vivado_block_design.tcl, package_fbc_ip.tcl |
| **profiles/** | zynq7020_sonoma.json |
| **prjxray-db/** | Open-source FPGA databases (artix7, kintex7, spartan7, zynq7) |

**Status:** Experimental. Uses prjxray segment databases for route resolution.
Has been used with XYZT MCP tools (hw_bridge, hw_bitstream_diff, etc.).
**NOT used for production bitstream** — Vivado builds the real bitstream.

### zynq-mkbootimage (open-source bootgen alternative)

| Path | `C:\Dev\tools\zynq-mkbootimage\` |
|------|---|
| Purpose | Generate BOOT.BIN without Xilinx tools |
| Status | Source present (Makefile + src/), not built |
| Alternative | Vivado's `bootgen.bat` does the same thing |

### Z3 SMT Solver

| Path | `C:\Dev\tools\z3\` |
|------|---|
| Version | 4.12.6 (64-bit) |
| Purpose | Used by fpga-toolchain for constraint solving |
| Status | Installed, libraries present (libz3.dll, .lib, .a) |

---

## 3. JTAG / Hardware Programming

### Our Custom Python Tools

#### fpga_jtag.py — PL Bitstream Programmer

| Path | `C:\Dev\xyzt_pico_Hardware\fpga_jtag.py` |
|------|---|
| Lines | 650 |
| Hardware | FT232H breakout (single-channel MPSSE) |
| Protocol | Direct JTAG bit-bang via pyftdi/MPSSE |
| What it does | Programs FPGA PL with .bit file via JTAG chain |
| Status | **WORKS** — programmed fbc_system.bit on March 13, 2026 |
| Performance | 5.5 seconds, 715 KB/s |
| Chain | TDI → ARM DAP (0x4BA00477, 4-bit IR) → XC7Z020 PL (0x23727093, 6-bit IR) → TDO |

**Known issues:**
- Requires pyftdi Python package
- JTAG speed: 6 MHz reliable
- J1 header is 2mm pitch Molex — must solder wires (dupont won't grip)

#### arm_loader.py — PS Firmware Loader

| Path | `C:\Dev\xyzt_pico_Hardware\arm_loader.py` |
|------|---|
| Lines | 2192 |
| Hardware | Same FT232H as fpga_jtag.py |
| Protocol | ARM Debug Access Port (DAP) via JTAG |
| What it does | Full PS initialization + ELF loading (replaces FSBL entirely) |
| Status | **WORKS** — loaded firmware March 17, 2026 (First Light) |
| Performance | ~11 seconds for 160KB ELF at 14.3 KB/s |

**What arm_loader.py initializes (in order):**
1. DDR PLL (FDIV=32, 1066 MHz)
2. ARM PLL (FDIV=40, 667 MHz CPU)
3. IO PLL (FDIV=30, 1000 MHz)
4. Clock dividers (CPU 6:4:2, DDR 3:2)
5. MIO pin mux (54 pins — UART0, SPI0, I2C0, GEM0, SD0, GPIO, MDIO)
6. UART0 baud rate
7. OCM remap to HIGH (0xFFFC0000)
8. PS-PL interface (level shifters, FCLK, deassert FPGA resets)
9. Load ELF segments to DDR
10. Set PC + CPSR, resume CPU

**Known issues:**
- MIO pin config is hand-tuned register values — error-prone, not auto-generated
- MIO 52-53 were wrong (I2C1 instead of MDIO) — FIXED March 19
- MIO 11 comment was wrong (said "UART0 TX", actually PHY_RESET) — FIXED
- No verification of register writes (write-and-pray)
- DDR timing values from "Sonoma FSBL via ONETWO" — not from Vivado's ps7_init
- Loading at 14.3 KB/s is slow (xsdb would be faster via DCC)
- Ethernet worked on March 19 ONLY because Sonoma's FSBL had previously configured MDIO correctly, and those SLCR values persisted through our JTAG PL programming

**Why this exists instead of xsdb:**
- Was built before discovering xsdb.bat is installed
- FT232H may not be recognized by hw_server (needs program_ftdi.bat to test)

### Xilinx JTAG Tools (INSTALLED, NEVER USED)

| Tool | What it replaces | Status |
|------|-----------------|--------|
| **hw_server.bat** | Talks to JTAG cables, serves debug connections | Never tried. Unknown if it sees FT232H |
| **xsdb.bat** | Interactive shell: program PL, init PS, load ELF, read memory | Never tried. Would replace BOTH Python scripts |
| **program_ftdi.bat** | Writes Xilinx cable descriptor to FTDI EEPROM | Never tried. May make FT232H visible to hw_server |

**What xsdb would give us:**
- `fpga` command — program bitstream (replaces fpga_jtag.py)
- `ps7_init` — auto-generated PS initialization (replaces arm_loader.py's hand-tuned registers)
- `dow` — download ELF to memory (replaces arm_loader.py's slow byte-by-byte)
- `con` — continue execution
- `mrd`/`mwr` — read/write memory (live debugging)
- `targets` — list JTAG chain
- All register values auto-generated from Vivado project (guaranteed correct)

**Blocker:** Need to test if program_ftdi can make our FT232H work with hw_server.
Alternative: Buy a Digilent JTAG-HS3 ($65) which is guaranteed compatible.

### Custom Rust FSBL (NEVER TESTED)

| Path | `C:\Dev\projects\FBC-Semiconductor-System\fsbl\src\main.rs` |
|------|---|
| Lines | 642 |
| Purpose | First Stage Boot Loader — SD card boot without Xilinx FSBL |
| What it does | DDR init → SD read → PCAP bitstream load → ELF parse → jump |
| Status | **NEVER TESTED** on hardware |

**Known gaps:**
- No PLL init (ARM/IO PLLs)
- No MIO pin mux
- No clock enables
- Only does DDR init
- DDR timing values "extracted from Sonoma FSBL via ONETWO"
- If we use Vivado's xsdb, this becomes unnecessary

---

## 4. Network / Packet Capture

### Npcap (raw packet capture)

| Component | Path |
|-----------|------|
| Runtime driver | `C:\Program Files\Npcap\npcap.sys` |
| SDK (headers + libs) | `C:\Dev\tools\npcap-sdk\` (Include/, Lib/) |

**Used by:** Host CLI (`host/src/fbc_protocol.rs`) for raw Ethernet 0x88B5 frames.
The host crate links against `wpcap.lib` / `Packet.lib`.

### WinEth (custom NDIS filter driver)

| Path | `C:\Dev\tools\wineth\` |
|------|---|
| Files | FbcEth.c, FbcEth.h, FbcEth.inf, FbcEth.sys |
| Purpose | Custom NDIS filter driver for raw Ethernet on Windows |
| Status | Built (x64/Release/FbcEth.sys), unclear if installed/loaded |

### Wireshark

| Path | `C:\Program Files\Wireshark\` |
|------|---|
| Version | 4.6.3 |
| Purpose | Packet analysis for debugging FBC Ethernet protocol |

### USBPcap

| Path | `C:\Program Files\USBPcap\` |
|------|---|
| Purpose | Capture USB traffic (debugging FT232H JTAG) |

---

## 5. Embedded / Hardware

### Raspberry Pi Pico SDK

| Path | `C:\Dev\tools\pico-sdk\` |
|------|---|
| Version | 2.2.0 |
| Purpose | XYZT Pico observer hardware firmware |
| Used by | `C:\Dev\xyzt_pico_Hardware\` |

### Raspberry Pi Imager

| Path | `C:\Program Files\Raspberry Pi Ltd\Imager\` |
|------|---|
| Purpose | Flash SD cards (for Pico, not for Zynq) |

### Rufus

| Path | `C:\Users\isaac\rufus.com` |
|------|---|
| Purpose | Create bootable USB drives |

---

## 6. AI / Reasoning Tools

### llama.cpp (local LLM inference)

| Path | `C:\Dev\tools\llama.cpp\` |
|------|---|
| CUDA | 13.1 (GPU accelerated) |
| Key binaries | llama-cli.exe, llama-server.exe, llama-quantize.exe |
| Models | qwen2vl, gemma3, llava, minicpmv |

---

## 7. Build Artifacts (What Exists Now)

### `build/` directory

| File | Size | What |
|------|------|------|
| `fbc_system.bit` | 3.9 MB | FPGA bitstream (synthesized + implemented) |
| `fbc_system.bin` | ~3.9 MB | Binary format (for SD boot) |
| `vivado/` | ~2 GB | Full Vivado project (synth + impl checkpoints) |
| `synth_utilization.rpt` | — | 12.6% LUT, 7.5% FF |
| `synth_timing.rpt` | — | Timing summary |
| `impl_utilization.rpt` | — | Post-route utilization |
| `impl_timing.rpt` | — | WNS: -1ns on AXI clock |
| `impl_power.rpt` | — | Power estimate |
| `impl_io.rpt` | — | Pin assignments |
| `impl_clocks.rpt` | — | Clock utilization (1 MMCM) |
| `impl_drc.rpt` | — | Design rule checks |
| `impl_error.log` | — | Implementation errors/warnings |

### Firmware ELF

| File | Size | Target |
|------|------|--------|
| `firmware/target/armv7a-none-eabi/release/fbc-firmware` | 453 KB | ARM Cortex-A9 bare-metal |

---

## 8. What's Working End-to-End (March 19, 2026)

| Step | Tool Used | Status |
|------|-----------|--------|
| RTL → Bitstream | `vivado.bat -mode batch -source scripts/build_bitstream.tcl` | **WORKS** |
| Bitstream → FPGA | `python fpga_jtag.py --device sonoma program build/fbc_system.bit` | **WORKS** |
| Firmware compile | `cd firmware && cargo build --release --target armv7a-none-eabi` | **WORKS** |
| Firmware → Board | `python arm_loader.py` | **WORKS** (with caveats — see known issues) |
| Host CLI → Board | `fbc-cli fbc discover` / `fbc-cli fbc status` etc. | **WORKS** (28 commands) |
| GUI build | `cd gui && npm run tauri dev` | **WORKS** |

### What's NOT working

| Step | Issue |
|------|-------|
| PHY link on fresh boot | MDIO pins misconfigured by arm_loader.py (fix in code, untested) |
| XADC readings | Sequencer init hangs on CMDFIFO (returns zeros) |
| SD card | Byte-write alignment fault without MMU (init disabled) |
| Firmware update OTA | Protocol exists, main.rs doesn't process pending requests |
| xsdb deployment | Never tried — FT232H compatibility unknown |

---

## 9. Recommended Path Forward

### Replace Custom JTAG Tools with Xilinx xsdb

**Current flow (fragile):**
```
fpga_jtag.py (650 lines)  →  Programs PL only
arm_loader.py (2192 lines) →  Hand-tuned PS init + ELF load
```

**Target flow (standard):**
```
hw_server.bat              →  Connects to JTAG cable
xsdb.bat                   →  Programs PL + auto PS init + ELF load
```

**Steps to get there:**
1. Test `program_ftdi.bat` on our FT232H — see if it can write Xilinx cable descriptor
2. If yes: `hw_server` will see it, `xsdb` replaces both Python scripts
3. If no: Buy Digilent JTAG-HS3 ($65) — guaranteed hw_server compatible
4. Export `.xsa` from Vivado project: `write_hw_platform -fixed -include_bit build/fbc_system.xsa`
5. Use xsdb's `ps7_init` with auto-generated register values (no more hand-tuning)

### Keep Custom Tools As Backup

The Python scripts work. Don't delete them — they're our fallback if xsdb/hw_server
has issues. But they should NOT be the primary deployment path.

### Custom FSBL Decision

`fsbl/src/main.rs` (642 lines) was never tested. Two options:
- **Use it:** Finish it (add PLL init, MIO mux), test, create BOOT.BIN with bootgen
- **Don't use it:** Use xsdb for dev, Xilinx FSBL (auto-generated) for production SD boot

For now: don't use it. xsdb for development, Vivado-generated FSBL for production.
