# CLAUDE.md — FBC Semiconductor System

Ground truth from code-level audit. Last major update: April 6, 2026.
Every claim verified by reading source.

---

## What This Is

Burn-in test system for semiconductor chips. ~44 Zynq 7020 FPGA boards per system.
**One GUI, profile-switched, controlling 5 tester system types.** Replaces three legacy Sonoma apps:

| Legacy App | What It Did | Our Replacement |
|------------|-------------|-----------------|
| **Unity** (Editor) | .tpf XML editor, .bim editor, PIN_MAP, .map/.lvl/.tim, PowerOn/Off | DeviceConfigPanel + PatternConverterPanel + dc_gen.c (generates all 7 file types) |
| **Everest** (Server) | NFS server, TCP :3000, .tpf→.tp conversion, RunVectors, ReadAnalog, multi-board orchestration, datalog CSV | CLI orchestrator (in progress), host crate (FBC + Sonoma clients) |
| **Bench App** (Manual) | SSH terminal, power control, pin debugging, vector load/run, ADC monitoring, EEPROM | CLI commands (33 FBC + 23 Sonoma, all verified on hardware) |

FBC is an **optimized Sonoma** (bare-metal Rust, raw Ethernet, compressed .fbc vectors) BUT also
the **Universal GUI/EDA** — profile-switched to support Sonoma, HX, XP-160/Shasta, and MCC systems.
Same GUI, different transport + file format per system, all feeding LRM v2 database.

**Owner:** Isaac Nudeton / ISE Labs

**LRM v2 exists as a SEPARATE project** at `C:\Dev\projects\Lab-Resource-Manager-v2-Isaac-\` —
full C server on port 8080, 104 REST routes, 21 tables, custom B-tree+WAL storage (33.7 MB db).
The `c-engine/lrm_*.c` files in THIS repo are FFI stubs, NOT the database itself.
Integration (FBC GUI → LRM API) is pending, but the server is built.

### Device Package Structure (Same Across ALL Systems)

Every customer device (Cisco C512, Microsoft Normandy, etc.) produces this file set:

| File | Format | Generator | Purpose |
|------|--------|-----------|---------|
| `PIN_MAP` | `"GPIO_IDX SIGNAL_NAME\n"` | `dc_gen_pinmap()` | 128/160 pin→signal mapping |
| `{device}.map` | `"SIGNAL = BANK_GPIO# ; DIR\n"` | `dc_gen_map()` | Signal→bank assignment |
| `{device}.lvl` | `"BANK <name> VOLTAGE=X.XXX\n"` | `dc_gen_lvl()` | Bank voltages + CMOS levels (70/30 rule) |
| `{device}.tim` | `"PERIOD=X DRIVE_OFF=X COMPARE=X\n"` | `dc_gen_tim()` | Timing parameters |
| `{device}.tp` | `"STEP PATTERN PATTERN_FILE LOOPS\n"` | `dc_gen_tp()` | Test plan steps (Sonoma) |
| `plan.json` | TestPlanDef JSON | `dc_gen_plan_json()` | FBC test plan (pattern_id, temp, clock, fail_action per step) |
| `PowerOn.sh` | Bash (MIO enable + DAC set) | `dc_gen_power_on()` | Sorted by sequence_order, DAC=(V/2.5)*4095 |
| `PowerOff.sh` | Bash (reverse of PowerOn) | `dc_gen_power_off()` | Reverse order shutdown |
| `{device}.bim` | 256-byte EEPROM XML | Manual / editor | Board Interface Module config |
| `{device}.tpf` | XML v5.0 test plan | Unity (legacy) / our editor | Full test plan (see .tpf schema below) |
| `vectors/*.hex` | 40B/vector binary | `gen_hex.c` | Legacy Sonoma vectors |
| `vectors/*.fbc` | Compressed binary | `gen_fbc.c` | FBC optimized vectors |

### .tpf XML Test Plan Schema (Production Format)

The `.tpf` is the master test plan file. Unity creates/edits it, Everest converts it to `.tp` for execution.
Our TestPlanEditor (784 lines) outputs JSON — `.tpf` XML parsing is a gap we need to close.

| Section | Contains | Editor Needs |
|---------|----------|-------------|
| `<TimeControl>` | Burn-in duration (D:H:M:S), ADC sample period | Time picker |
| `<ThermalControl>` | Sensor type (Case_30K NTC / Linear diode), sensor pin | Dropdown |
| `<BimType>` | .bim file path, device name, type ID | File picker |
| `<Power>` | Named supply list (16+ entries) | Supply table |
| `<Global><References>` | IO bank voltages (4 banks) | 4 voltage inputs |
| `<Global><ADC>` | Monitoring pins: name, type, min/max shutdown, formula | Table editor |
| `<Global><Power>` | Per-supply V+I limits, autoadj flag, scale formula | Table editor |
| `<TestSteps>` | Ordered steps, each with temp/vectors/power/timing | Step list |

Each `<TestStep>` has: Temperature (setpoint, UL, LL, ramp), Duration (repeats, freq, option code),
PinStimuli (PULSE/NPULSE/INPUT edge timing), Pattern (.hex file list), Sequencing (supply ramps).

---

## CRITICAL: Read Before Touching Anything

### Terminology
| Term | Meaning |
|------|---------|
| VICOR | 6 high-current core power supplies |
| LCPS | Low Current Power Supply (PMBus) |
| BIM | Board Interface Module — separate board (e.g. calibration BIM = blue board) with EEPROM, NTC, FETs, interposer. Connects to controller board (green/Zynq) via J3/J4/J5 |
| DUT | Device Under Test (the chip being burned in) |
| Fast Pins | gpio[128:159], direct FPGA I/O, 1-cycle latency |
| BIM Pins | gpio[0:127], through interposer, 2-cycle latency |
| ONETWO | Our methodology: decompose to invariants (ONE), then build (TWO) |
| `.hex` | Legacy Sonoma format — 40 bytes/vector, uncompressed |
| `.fbc` | **FBC compressed format** — 1-21 bytes/vector (VECTOR_ZERO, VECTOR_RUN, VECTOR_SPARSE opcodes) |

### Source of Truth Files
| What | File | Why |
|------|------|-----|
| FBC instruction set | `rtl/fbc_pkg.vh` | Defines all opcodes, widths, parameters |
| Instruction execution | `rtl/fbc_decoder.v` | State machine that runs FBC bytecode |
| FPGA integration | `rtl/system_top.v` | Zynq PS7 + all AXI peripherals |
| Protocol wire format | `firmware/src/fbc_protocol.rs` | 79 commands, 13 subsystems, all payload structs |
| Register access | `firmware/src/regs.rs` | All FPGA register offsets (verified vs RTL) |
| Device DNA / MAC | `rtl/axi_device_dna.v` + `firmware/src/hal/dna.rs` | Silicon ID → unique MAC per board |
| Main firmware loop | `firmware/src/main.rs` | Boot, networking, command dispatch, slot/plan integration |
| SD + DDR double-buffer | `firmware/src/ddr_slots.rs` | SD pattern library (256 max) + DDR double-buffer (A/B regions, 252+256MB), non-blocking chunked loading |
| Test plan executor | `firmware/src/testplan.rs` | Autonomous burn-in (96 steps max), per-step temp/clock, checkpoint persistence |
| GUI protocol client | `gui/src-tauri/src/fbc.rs` | Socket, types, constants |
| GUI state machine | `gui/src-tauri/src/state.rs` | All command send/recv, payload parsing |
| JTAG programmer | `C:\Dev\xyzt_pico_Hardware\fpga_jtag.py` | Multi-device JTAG, programs PL via FT232H |
| Build script | `scripts/build_bitstream.tcl` | Full Vivado build: PS7 + synth + impl + bitgen |

---

## ✅ Pattern Converter — All Output Formats Complete (March 2026)

**Full pipeline now works:**
```
ATP/STIL/AVC + PIN_MAP
    ↓
C Engine (gui/src-tauri/c-engine/pc/)
    ↓
├── gen_hex.c  → .hex (40 bytes/vector, legacy Sonoma)
├── gen_seq.c  → .seq (test sequence text)
└── gen_fbc.c  → .fbc (compressed FBC: 1-21 bytes/vector, 4.8-710x compression)
```

| Converter | Input | Output | Status |
|-----------|-------|--------|--------|
| **C Engine** (`c-engine/pc/`) | ATP/STIL/AVC | `.hex` + `.seq` | ✅ Complete |
| **gen_fbc.c** (`c-engine/pc/`) | PcPattern IR | `.fbc` | ✅ Complete (March 2026) |
| **Rust Compiler** (`host/src/vector/`) | `.fvec` (text) | `.fbc` | ✅ Complete |

**gen_fbc.c** implements the same compression algorithm as the Rust compiler (compiler.rs),
byte-compatible: ZERO(1B) / ONES(1B) / RUN(1+4B) / SPARSE(1+1+NB, crossover=15) / FULL(1+20B).
CRC32 over header+pin_config+data (IEEE 802.3). Tauri command `pc_convert` accepts `fbc_output`
parameter. Frontend has `.fbc OUT` file picker in Pattern Conversion tab.

### Smart Parsers — Automatic Inference (No Manual JSON Required)

**STIL Smart Parser** (`parse_stil_smart.c`) — 3-phase inference:
1. Parse Signals → infer pin types from names:
   - `_TCK`, `_CLK` → PULSE_POS (clock)
   - `_TDO`, `_DOUT` → MONITOR (output sense)
   - `_TDI`, `_TMS`, `_EN`, `_CS` → IO (bidirectional)
2. Parse SignalGroups → link signals: `"JTAG_GROUP = 'TCK + TMS + TDI + TDO'"` → all tagged "JTAG"
3. Parse Timing → extract clock period from waveform patterns

**AVC Smart Parser** (`parse_avc_smart.c`) — behavioral inference:
- Temperature from timing set: `tp1`→100°C, `tp2`→125°C, `tp3`→150°C, `room`→25°C, `cold`→-40°C
- Test type from timing set: `burn`→burn-in, `func`→functional, `scan`→scan, `atpg`→ATPG
- Pin behavior from vector patterns: clock if 'C'>10%, monitor if 'L'/'H'>50%, else IO
- Pattern name auto-generated: `"AVC_tset_gen_tp1_T100C"`

**Pin Map Parser** (`parse_pinmap.c`) — 3 formats auto-detected:
1. Board PIN (Sonoma): `B13_GPIO0 PAD_A_RSTN;` → channel from GPIO index
2. Direct GPIO: `0 signal_name` → channel from first token
3. burnIn.cfg (VelocityCAE): `PINLIST...END PINLIST` 6-column format with ATE_PINNAME + CHANNEL

### Device File Generation — 7 Output Types (`dc_gen.c`)

`dc_gen.c` is **profile-agnostic** — works with ANY system profile (Sonoma, HX, MCC, etc.).
Level derivation: VIH=Vbank×0.7, VIL=Vbank×0.3, VOH=Vbank×0.8, VOL=Vbank×0.2 (CMOS 70/30).
Power scripts: insertion-sort by `sequence_order`, DAC mapping = `(voltage / 2.5) * 4095`.

### Pin Import — CSV/Excel/PDF Extraction + Cross-Verification

**Solved:** Engineers import pin tables from datasheets, edit inline, verify against a second source.

| Source | Input | Output | Status |
|--------|-------|--------|--------|
| CSV/Excel/PDF → Pin Table | .csv/.xlsx/.pdf | Editable pin table + device JSON | ✅ Complete |
| Pin Table → Device Config | Extracted pins | PIN_MAP + .map + .lvl + .tim + .tp + PowerOn/Off.sh | ✅ Complete |
| Cross-Verification | 2x pin tables | Mismatch report (channel/voltage/direction) | ✅ Complete |

**CSV extraction** (`pin_extractor.rs` + `dc_csv.c`):
- Auto-detect delimiter: tabs > semicolons > commas (ignoring quoted fields)
- Auto-detect header row: scan first 10 rows for ≥2 recognized column names
- Recognized columns: signal/pin_name/net, channel/gpio/pin#, direction/dir/io, voltage/vio/level, group/bank/domain
- Supply detection: signal contains "CORE"/"VDD"/"VOUT"/"VCC"/"SUPPLY" → treated as power supply row
- Fallback: positional (col0=signal, col1=channel, col2=dir, col3=voltage) if no header found

**Excel extraction**: calamine crate → same column detection as CSV (shared `parse_string_rows_into_table()`)

**PDF extraction** (two strategies):
1. **Tabular** (datasheets): Find header line with Pin/Signal/Channel keywords, parse fixed-width rows
2. **Scattered** (schematics): Pattern-match `"Pin N SIGNAL"` pairs scattered in drawing text.
   Works because PCB tools (Altium/Eagle/KiCad) embed text as real PDF text, not raster. Fuzzier — more
   warnings, more user editing expected.

**Cross-verification** (`cross_verify()`):
- Primary source = ground truth, secondary = validation
- Match by signal name (case-insensitive)
- Report: channel mismatch, direction mismatch, voltage mismatch (tolerance: |v1-v2| > 0.01V)
- Report: signals in secondary but missing from primary
- Result: `VerificationResult { mismatches, match_count, mismatch_count }`

**Full pipeline:**
```
Source file (CSV/Excel/PDF) → extract_pin_table() [Rust]
    → ExtractedPinTable (editable in frontend)
    → (optional) cross_verify() against secondary source (schematic PDF, second CSV)
    → to_device_json() → temp device.json
    → dc_parse_device() [C] → DcDeviceIR
    → dc_gen_all() [C] → 7 device files (PIN_MAP, .map, .lvl, .tim, .tp, PowerOn.sh, PowerOff.sh)
```

**Files:**
- Rust: `gui/src-tauri/src/pattern_converter/pin_extractor.rs`
- C: `dc_csv.c` (CSV parsing), `dc_gen.c` (file generation), `dc_json.c` (device JSON parsing)
- Commands: `extract_pin_table`, `verify_pin_tables`, `generate_from_extracted`
- Dependencies: calamine (Excel), pdf-extract (PDF), csv (Rust CSV)

### C Engine API Surface (`c-engine/pc/`)

The C pattern converter is a **zero-dependency C11 library** compiled into the Tauri binary via `build.rs` (cc crate).
All headers: `pc.h` (pattern conversion) + `dc.h` (device config generation).

**Build:** `build.rs` compiles these C sources into `libpattern_converter.a`:
```
ir.c, crc32.c, parse_atp.c, parse_pinmap.c, parse_stil_smart.c, parse_avc_smart.c,
gen_hex.c, gen_seq.c, gen_fbc.c, dc_json.c, dc_gen.c, dc_csv.c,
dll_api.c, dc_api.c, vendor/cJSON.c
```

**Core IR (ir.c):**
```c
void pc_pattern_init(PcPattern *p, const char *name);
void pc_pattern_free(PcPattern *p);
int  pc_pattern_add_signal(PcPattern *p, const char *name);
int  pc_pattern_add_vector(PcPattern *p, const PcVector *v);
PinState pc_char_to_state(char c);        // '0'→PS_DRIVE_0, 'H'→PS_EXPECT_H, etc.
char     pc_state_to_char(PinState s);
void pc_encode_vector(const PcVector *v, PcHexVector *out);  // → 40-byte hex format
```

**Parsers:**
```c
int pc_parse_atp(PcPattern *p, const char *path);            // ATP format
int pc_parse_stil_smart(PcPattern *p, const char *path);     // STIL (infers pin types, groups, timing)
int pc_parse_avc_smart(PcPattern *p, const char *path);      // AVC (infers test type, temperature)
int pc_load_pinmap(PcPattern *p, const char *path);          // Signal→channel mapping
void pc_apply_identity_map(PcPattern *p);                    // Signal index N → channel N
```

**Generators:**
```c
int pc_gen_hex(const PcPattern *p, const char *path, int append_crc);        // → .hex (40B/vector)
int pc_gen_seq(const PcPattern *p, const char *path, const char *atp_name);  // → .seq text
int pc_gen_fbc(const PcPattern *p, const char *path, uint32_t vec_clock_hz); // → .fbc compressed
```

**DLL API (dll_api.c) — handle-based, FFI-safe:**
Static pool of 16 `PcPattern` slots. Integer handles. No structs cross boundary.
```c
PC_API int         pc_create(void);                                    // → handle (0-15)
PC_API void        pc_destroy(int h);
PC_API int         pc_dll_load_pinmap(int h, const char *path);
PC_API int         pc_dll_load_input(int h, const char *path, int format);  // FMT_AUTO/ATP/STIL/AVC
PC_API int         pc_dll_convert(int h, const char *hex_path, const char *seq_path);
PC_API int         pc_dll_gen_fbc(int h, const char *fbc_path, uint32_t vec_clock_hz);
PC_API int         pc_dll_num_signals(int h);
PC_API int         pc_dll_num_vectors(int h);
PC_API const char *pc_dll_last_error(int h);
PC_API const char *pc_dll_version(void);
```

**Device Config API (dc_api.c) — same handle pattern:**
Static pool of 16 `DcHandle` slots. Generates device config files from JSON.
```c
PC_API int         dc_create(void);                                    // → handle (0-15)
PC_API void        dc_destroy(int h);
PC_API int         dc_load_profile(int h, const char *path_or_name);   // built-in name OR file path
PC_API int         dc_load_device(int h, const char *path);            // device config JSON file path
PC_API int         dc_validate(int h);                                 // check constraints
PC_API int         dc_generate(int h, const char *out_dir);            // all files at once
PC_API int         dc_gen_file(int h, const char *output_dir, int file_type);  // single file (DC_FILE_PINMAP=0, etc.)
PC_API int         dc_num_channels(int h);                             // channel count from profile
PC_API int         dc_num_supplies(int h);                             // supply count from profile
PC_API int         dc_num_steps(int h);                                // test step count from device
PC_API const char *dc_profile_name(int h);                             // loaded profile name
PC_API const char *dc_last_error(int h);
// NOT in DLL API: dc_load_csv (internal only via dc_parse_csv in dc_csv.c)
// NOT in DLL API: dc_get_builtin_profile(name) — returns const char* JSON, internal use
```

**Return codes:** `PC_OK(0)`, `PC_ERR_FILE(-1)`, `PC_ERR_PARSE(-2)`, `PC_ERR_ALLOC(-3)`,
`PC_ERR_PINMAP(-4)`, `PC_ERR_HANDLE(-5)`, `PC_ERR_FORMAT(-6)`, `PC_ERR_WRITE(-7)`.

### Rust FFI Layer (`src/pattern_converter/pc_ffi.rs`)

Wraps the C DLL API in safe Rust. `PcHandle` struct with RAII (calls `pc_destroy` on drop).
```rust
pub struct PcHandle { handle: i32 }
impl PcHandle {
    pub fn new() -> Result<Self, String>;
    pub fn load_input(&self, path: &str, format: Option<&str>) -> Result<(), String>;
    pub fn load_pinmap(&self, path: &str) -> Result<(), String>;
    pub fn convert(&self, hex_path: &str, seq_path: &str) -> Result<(), String>;
    pub fn gen_fbc(&self, fbc_path: &str, vec_clock_hz: u32) -> Result<(), String>;
    pub fn num_signals(&self) -> i32;
    pub fn num_vectors(&self) -> i32;
}
```

### Pattern Converter Tauri Commands

| Command | Parameters | Output |
|---------|-----------|--------|
| `pc_convert` | `input_path, pinmap_path?, hex_output?, seq_output?, fbc_output?, format?, vec_clock_hz?` | `{signals, vectors, hex_path?, seq_path?, fbc_path?}` |
| `dc_generate_config` | `device_json, profile, output_dir` | generation result |
| `dc_generate_file` | `device_json, profile, file_type, output_path` | single file result |
| `dc_generate_from_csv` | `csv_path, profile, output_dir` | generation result |
| `extract_pin_table` | `file_path` | `ExtractedPinTable` (signals, supplies, warnings) |
| `verify_pin_tables` | `primary_path, secondary_path` | `VerificationResult` (mismatches) |
| `generate_from_extracted` | `data: ExtractedPinTable, profile, output_dir` | generation result |

### .fbc Binary Format Reference

```
HEADER (32 bytes):
  [0:4]   magic        = 0x00434246 ("FBC\0", LE)
  [4:6]   version      = 1
  [6]     pin_count    = 160
  [7]     flags        = bit 0: has thermal profile (FBC_FLAG_THERMAL_PROFILE = 0x01)
  [8:12]  num_vectors  = total uncompressed vector count
  [12:16] compressed_size = bytes of compressed data (includes thermal profile after OP_END)
  [16:20] vec_clock_hz = vector clock frequency
  [20:24] crc32        = IEEE 802.3 CRC over header+pin_config+data (this field=0 during calc)
  [24:28] _reserved[0:4] = thermal segment count (u32 LE) if flags & 0x01, else 0
  [28:32] _reserved[4:8] = 0

PIN_CONFIG (80 bytes):
  160 pins × 4 bits = 640 bits = 80 bytes
  Pin types: 0=BIDI, 1=PULSE_POS, 2=PULSE_NEG, 3=MONITOR, 4=SUPPLY

COMPRESSED DATA (variable):
  OP_VECTOR_FULL   0x01  1+20B  (20-byte raw vector)
  OP_VECTOR_SPARSE 0x02  1+1+NB (count + changed bytes, crossover at N=15)
  OP_VECTOR_RUN    0x03  1+4B   (repeat count, LE u32)
  OP_VECTOR_ZERO   0x04  1B     (all-zero vector)
  OP_VECTOR_ONES   0x05  1B     (all-ones vector)
  OP_VECTOR_XOR    0x06  1+20B  (XOR with previous vector — defined, not yet emitted by encoder)
  OP_END           0x07  1B     (stream terminator)

THERMAL_PROFILE (after OP_END, if flags & 0x01):
  N × 8 bytes, where N = _reserved[0:4]:
    [0:4]  vector_offset    (u32 LE — starting vector index)
    [4]    avg_toggle_rate  (0-160)
    [5]    avg_active_pins  (0-160)
    [6]    power_level      (0=Low, 1=Medium, 2=High)
    [7]    reserved         (0)
  Segments every 1024 vectors. Generated during compression (XOR + popcount, zero extra cost).
  Firmware loads at .fbc load time → feedforward thermal schedule before first vector fires.
```

Compression ratio: 4.8x (random data) to 710x (uniform vectors). Byte-compatible between
C compiler (`gen_fbc.c`) and Rust compiler (`compiler.rs`). CRC32 uses same polynomial as `crc32.c`.
Thermal profiling: both compilers emit identical segment data (25/25 tests pass).

---

## Multi-System Profiling Architecture

This app supports 5 tester system types. The profiling concept exists at **3 layers**:

### Layer 1: Inventory Database (lrm_schema.h)
```c
// gui/src-tauri/c-engine/lrm_schema.h:28-31
typedef enum {
    SYS_HX=0, SYS_SONOMA=1, SYS_XP160=2, SYS_MCC=3, SYS_SHASTA=4, SYS__COUNT=5
} SystemType;
```
- Used in `System` struct (every physical machine record has a type)
- Used in `HardwareType` struct (`for_system_type` restricts hardware to specific systems)
- String table: `schema.c:12` — `{"HX","Sonoma","XP-160","MCC","Shasta"}`
- Runtime branching: `inventory.c:187` — location tree generation differs per system type

### Layer 2: Pattern Converter Profiles (dc.h / dc_json.c)
```c
// gui/src-tauri/c-engine/pc/dc.h:94-107
typedef struct {
    char       name[DC_MAX_NAME];      // "Sonoma", "HX", "XP-160/Shasta", "MCC"
    int        total_channels;         // 128 (Sonoma/MCC), 160 (HX/XP-160 per axis)
    DcGpioBank banks[DC_MAX_BANKS];    // Pin bank layout
    int        num_banks;
    DcCoreHw   cores[DC_MAX_SUPPLIES]; // Power supply hardware
    int        num_cores;
    // ... firmware_path, vector_dir, timing defaults
} DcTesterProfile;
```
- Built-in profiles embedded as JSON strings in `dc_json.c`
- **Sonoma**: fully implemented (banks, supplies, firmware paths, timing)
- **HX, XP-160/Shasta, MCC**: structural stubs (channels, banks, supplies, timing present; `firmware_path` and `vector_dir` empty — need transport-specific settings)
- Lookup: `dc_get_builtin_profile("sonoma")` → returns JSON, parsed into struct
- See `PROFILE-INSTRUCTIONS.md` for completing HX/XP-160/MCC profiles

### Layer 3: Host CLI Transport (host/src/bin/cli.rs)
```rust
// host/src/bin/cli.rs:48-76
enum Commands {
    Fbc { ... },     // Raw Ethernet 0x88B5 (bare-metal FPGA)
    Sonoma { ... },  // SSH + ELF binaries (Linux Zynq)
}
```
- Transport selection is implicit via CLI subcommand
- `SonomaClient` (SSH) vs `FbcClient` (raw Ethernet) in host/src/
- **Missing link:** board's SystemType should auto-select transport + profile

### System Specs (from FSHC tester.rs + hardware verification)

| System | Channels | Axes | Supplies | Timing | Thermal | Transport |
|--------|----------|------|----------|--------|---------|-----------|
| **Sonoma** | 128 | 1 | 6 VICOR | 100ps/200MHz | ARM PID → BIM FETs (no external Watlow) | SSH (Linux) |
| **HX** | 160/axis | 4 | 16 RMA5608 | 200ps/200MHz | RMA5608 4-zone | INSPIRE |
| **XP-160/Shasta** | 160/axis | 8 | 32 RMA5608 | 200ps/200MHz | RMA5608 8-zone | INSPIRE |
| **MCC** | 128 | 1 | 8 | 1ns/50MHz | Watlow 1-zone | Modbus TCP |
| **FBC** (future) | 160 | 1 | 6 VICOR | 100ps/200MHz | ARM crystallization v2 → BIM FETs | Raw Ethernet |

HX and XP-160/Shasta use **the same driver** — Shasta is just newer. Only difference = axis count.
Per-axis layout identical: 96 drive + 60 monitor + 4 reserved = 160 channels.

### Transport Details Per System

**Sonoma (IMPLEMENTED):** SSH + ELF binaries on Linux Zynq.
- `SonomaClient` in `host/src/sonoma.rs` — 34 pub methods via SSH
- ELF tools: `RunSuperVector.elf`, `linux_run_vector.elf`, `linux_load_vectors.elf`,
  `linux_IO_PS.elf`, `linux_VICOR.elf`, `linux_pmbus_PicoDlynx.elf`
- NFS serves device packages to boards, TCP :3000 for orchestration
- Vector execution: `.seq` + `.hex` loaded via `linux_load_vectors.elf`, run via `RunSuperVector.elf`

**FBC (IMPLEMENTED):** Raw Ethernet 0x88B5, bare-metal ARM firmware.
- `FbcClient` in `host/src/lib.rs` — 47 pub methods (44 protocol + 3 utility), all tested on hardware
- Direct AXI register access, DMA vector upload, compressed .fbc format
- No OS, no SSH, no NFS — everything over single Ethernet frame protocol

**HX / XP-160/Shasta (NOT IMPLEMENTED — enum + specs only):**
- Transport: INSPIRE (Aehr/Incal proprietary protocol)
- Software: INSPIRE v4.9 (HX) / INSPIRE XP8 v1.3.16 (XP-160)
- Pattern tool: `PatConvert.exe` (converts STIL/AVC/APS → INSPIRE format)
- Power train: RMA5608 (provides both power distribution AND thermal zones)
- Script syntax: `POWER.SET [rail], [voltage], [current_max]` with semicolon comments
- Vector formats: STIL, AVC, APS (ISE Pattern Editor ASCII)
- Pattern memory: 650K vectors per axis
- Connectors: RMA5608 Power Train (160-pin) + XPS-4/XPS-8 Data (160-pin)
- **What we'd need:** Reverse-engineer INSPIRE wire protocol, or get Aehr SDK docs

**MCC (NOT IMPLEMENTED — enum + specs only):**
- Transport: Modbus TCP (standard industrial protocol — straightforward to implement)
- Thermal: Watlow controller via Modbus TCP/IP (1 zone, -40°C to 150°C, 5°C/min ramp) — NOTE: MCC may actually use an external Watlow unlike Sonoma
- Unique features: 16 pattern zones, PLC integration, DB2 backend
- Timing: 1ns resolution / 50MHz max (5x coarser than HX/Sonoma)
- **What we'd need:** Modbus register map for Watlow + MCC controller command set

### What's Complete vs Missing

| Layer | Sonoma | HX | XP-160/Shasta | MCC | FBC |
|-------|--------|----|---------------|-----|-----|
| LRM SystemType enum | ✅ | ✅ | ✅ | ✅ | ❌ (add SYS_FBC=5) |
| C Engine profile (dc_json.c) | ✅ | ⚠️ stub | ⚠️ stub | ⚠️ stub | ❌ |
| GUI dropdown | ✅ | ❌ | ❌ | ❌ | ❌ |
| Host transport | ✅ SSH | ❌ | ❌ | ❌ | ✅ Raw Ethernet |
| Pattern converter output | ✅ .hex/.seq/.fbc | ❌ | ❌ | ❌ | ✅ .fbc |

### Key Files for Multi-System Work

| File | Purpose |
|------|---------|
| `gui/src-tauri/c-engine/lrm_schema.h` | SystemType enum, System/HardwareType structs |
| `gui/src-tauri/c-engine/schema.c` | system_type_str(), enum-to-string |
| `gui/src-tauri/c-engine/inventory.c` | System-specific location tree generation |
| `gui/src-tauri/c-engine/pc/dc.h` | DcTesterProfile struct, limits (DC_MAX_SUPPLIES etc.) |
| `gui/src-tauri/c-engine/pc/dc_json.c` | Built-in profile JSONs + dc_get_builtin_profile() |
| `gui/src-tauri/c-engine/pc/dc_gen.c` | File generators (profile-agnostic, work with any profile) |
| `gui/src-tauri/src/pattern_converter/pc_ffi.rs` | Rust→C FFI (passes profile name as string) |
| `gui/src-tauri/src/lib.rs` | Tauri commands (profile parameter flows through) |
| `gui/src/components/PatternConverterPanel.tsx` | Profile dropdowns (currently Sonoma-only) |
| `host/src/bin/cli.rs` | Transport subcommands (Fbc vs Sonoma) |
| `host/src/types.rs` | SonomaStatus, RunResult (system-specific types) |
| `PROFILE-INSTRUCTIONS.md` | Implementation guide for adding HX/XP-160/MCC profiles |
| `C:\Dev\FSHC - Hardware\fshc\crates\fshc-platform\src\tester.rs` | Complete Rust TesterProfile definitions for all systems |

---

## Directory Structure (What's Real)

```
├── rtl/                   # 16 Verilog modules (VERIFIED, programmed on hardware)
│   ├── system_top.v       # Top: PS7 + clk_wiz + fbc_top + fbc_dma + 3×error_bram + device_dna
│   ├── fbc_top.v          # FBC core: io_config + io_bank + axi_stream_fbc + fbc_decoder + vector_engine + error_counter + axi_fbc_ctrl + axi_vector_status
│   ├── fbc_dma.v          # AXI DMA: HP0 DDR read → 256-bit AXI-Stream to fbc_decoder
│   └── (13 more)          # io_cell, clk_gen, error_bram, etc.
├── tb/                    # Testbenches
├── constraints/           # Pin constraints (.xdc)
├── firmware/              # ARM Cortex-A9 bare-metal Rust (30 source files)
│   └── src/
│       ├── main.rs        # Entry, boot, main loop
│       ├── fbc_protocol.rs # 79 commands across 13 subsystems (+ MIN_MAX 0xF2/F3, IO_BANK 0x35/36)
│       ├── regs.rs        # FPGA register access
│       ├── dma.rs         # AXI DMA + FbcStreamer + stream_from_ddr()
│       ├── ddr_slots.rs   # SD pattern library (256 max) + DDR double-buffer (A/B regions, non-blocking chunked load)
│       ├── testplan.rs    # Autonomous burn-in state machine (PlanExecutor)
│       ├── analog.rs      # 32-ch ADC (XADC + MAX11131)
│       ├── net.rs         # Zynq GEM Ethernet driver
│       └── hal/           # 16 hardware drivers
├── gui/                   # Tauri + React + Three.js
│   ├── src/               # React frontend
│   └── src-tauri/         # Rust backend
├── host/                  # CLI + shared library (33 FBC + 23 Sonoma commands)
├── reference/             # Old 2016 kzhang_v2 design (READ ONLY reference)
├── docs/                  # Architecture analysis docs
├── fsbl/                  # First Stage Boot Loader
├── tools/                 # Utilities (routing verify, rawwrite)
├── testplans/             # Test plan examples
├── onetwo.c               # ONETWO reasoning scaffold
└── CLAUDE.md              # THIS FILE
```

### Hardware Status (March 2026)

| Component | Status | Notes |
|-----------|--------|-------|
| **PL (FPGA)** | ✅ **v7d April 6** | clk_wiz IP replaces hand-rolled clk_ctrl. WNS=+0.028ns. freq_counter removed. 7 AXI peripherals + clk_wiz. |
| **PS (ARM Firmware)** | ✅ **April 6** | clk_wiz DRP, headroom thermal, SD double-buffer, ADC CS2 fix, VICOR current readback. 0 warnings. |
| **VICOR GPIO** | ✅ FIXED | SLCR MIO mux for 6 VICOR pins `[0, 39, 47, 8, 38, 37]` — all 6 including MIO0 (Core 1) |
| **CLI Live Test** | ✅ **12/12 tested March 30** | All respond. Clock configure pending v7d hardware test. 38 FBC + 23 Sonoma CLI commands. |
| **LEDs** | ❌ NONE | Board has NO firmware-controllable LEDs. Only 3.3V (green, power) + DONE (blue, FPGA config) — both hardware-driven |
| **Error BRAM** | ✅ WIRED | 3× BRAMs at 0x4009_0000, protocol handler added |
| **clk_ctrl → clk_wiz** | ✅ **REPLACED April 6** | Hand-rolled clk_ctrl had AXI runtime crash (optimizer removed read path). Replaced with Vivado clk_wiz IP (DRP). v7d bitstream: WNS=+0.028ns. Pending hardware verification. |
| **SD Card** | ✅ FIXED | `sd_init_ok` guard added — returns error when no SD card instead of crashing |
| **Device DNA** | ✅ **VERIFIED March 25** | Unique MAC `00:0A:35:C6:B4:2A` from silicon DNA. Firmware reads FBC_CTRL VERSION at 0x4004_001C to confirm peripheral. |
| **XADC Temp** | ✅ **VERIFIED March 25** | Reads 39.5°C (was -220°C before u64 overflow fix) |
| **DMA** | ✅ WIRED | `fbc_dma.v` instantiated, used by `FbcStreamer` |

### What's NOT Here
- `archive/` — DELETED (was OBI-1 physics, wrong repo)
- `learning/` — DELETED (stale HTML tutorials)
- `STATUS.md` — DELETED (stale percentages)
- `TODO.md` — DELETED (outdated 677-line roadmap)

---

## Architecture: What Actually Works End-to-End

```
GUI (Tauri) ──Raw Ethernet 0x88B5──▶ Firmware (bare-metal Rust on Cortex-A9)
                                         │
                                    AXI-Lite bus (GP0)
                                         │
       ┌──────────┬──────────┬───────────┼───────────┬──────────┬──────────┬──────────┐
       ▼          ▼          ▼           ▼           ▼          ▼          ▼          ▼
  fbc_ctrl   io_config   vec_status  clk_wiz   (removed)    err_bram  device_dna  fbc_dma
  0x4004_0   0x4005_0    0x4006_0   0x4008_0   0x4007_0    0x4009_0  0x400A_0    0x4040_0
  ctrl/stat  pin types   error/vec  freq_sel   4ch meas    3×BRAM    57-bit DNA  HP0 DMA
       │          │          ▲           │                     ▲          │
       ▼          ▼          │           ▼                     │          ▼
    fbc_dma ──▶ axi_stream_fbc ──▶ fbc_decoder ──▶ vector_engine ──▶ io_bank ──▶ 160 Pins
    (HP0 DDR)   (256→64+128)      (7 opcodes)    (repeat+errors)   (128 BIM + 32 fast)
```

**Protocol:** Raw Ethernet frames, EtherType 0x88B5, 8-byte FbcHeader (magic=0xFBC0, seq, cmd, flags, length), big-endian payloads.
**JTAG chain:** TDI → ARM DAP (0x4BA00477) → XC7Z020 PL (0x23727093) → TDO
**Bitstream:** `build/fbc_system.bit` (3.9 MB) — programmed and verified on real hardware March 13, 2026.

---

## Open Bugs

6. **LOOP_N non-functional** — `fbc_decoder.v:128-132`: no instruction buffer to replay loop body. Unroll in bytecode.
10b. **Phase clocks hardwired** — `clk_gen.v` CLKOUT5/6 fixed at 50MHz@90/180. Don't follow freq_sel.
11. **5 opcodes unimplemented** — SYNC, IMM32, IMM128, PATTERN_SEQ, SET_BOTH → S_ERROR. Use SET_PINS + SET_OEN instead.
16. ~~**Dead code in net.rs**~~ — **FIXED March 25**. TcpServer/UdpPacket/RawFrame were defined but never instantiated in the firmware. Removed. Note: Sonoma support lives in the HOST crate (`sonoma.rs` SonomaClient via SSH) and GUI profiles — NOT in the firmware. The firmware builds exactly one binary: raw Ethernet FBC protocol.
17. **FreqCounter never used** — `axi_freq_counter.v` at 0x4007_0000, firmware never reads it.
18. **PCAP module unused** — `hal/pcap.rs` (358 lines), not called.
19. **Firmware update untested** — BEGIN/CHUNK/COMMIT pipeline exists but never tested on hardware.
29. ~~**Rust compiler CRC bug**~~ — **FIXED March 25**. `format.rs:write_to()` zeroed CRC in output — every .fbc from Rust compiler had CRC=0. Firmware would have rejected them. Now calculates and writes correct CRC.
30. ~~**Rust compiler emit_run off-by-one**~~ — **FIXED March 25**. When vec == prev, `emit_run` skipped emitting the vector but used `count-1` for RUN. Fixed: uses `count` when vector not emitted.
20. **Vector data truncation** — `fbc_decompress.rs:259` copies only 16/20 bytes (128 bits) per vector. Fast pins (128-159) never driven by test vectors. Matches `axi_stream_fbc.v:59` 128-bit bus width. Needs architectural decision: document as constraint, firmware workaround, or RTL fix.
21. **PATTERN_REP -1 undocumented** — `fbc_decompress.rs:273` subtracts 1 from repeat count. Convention not documented in `fbc_pkg.vh`. Off-by-one risk for encoder authors.
22. ~~**clk_ctrl AXI crash**~~ — **FIXED April 6**. Root cause: Vivado optimizer removed hand-rolled AXI read path. Replaced with Vivado clk_wiz IP (DRP). v7d bitstream: WNS=+0.028ns. Pending hardware verification.
23. ~~**SD commands crash board**~~ — **FIXED March 24**. Root cause was uninitialized SDHCI (not byte writes as originally thought). `sd_init_ok` guard added in `main.rs:394-420`.
24. ~~**XADC temp wrong**~~ — **FIXED and VERIFIED March 25**. u32 overflow in `xadc.rs:242` (`as u32` → `as u64`). `raw * 503975` overflows u32 for raw > 8522. Hardware reads 39.5°C (was -220°C). DUT thermal from MAX11131 ch 22-23 (THERM_CASE/THERM_DUT NTC on calibration BIM). Thermal controller v2 (`thermal.rs`) wired into main.rs.

All previously-listed bugs (1-5, 7-10, 12-15) are **FIXED**. Bug 20a (Error BRAM cycle offset) **FIXED March 24** — `regs.rs` read 0x1C/0x20 instead of 0x18/0x1C.
25. ~~**MAC collision**~~ — **FIXED March 24**. All Zynq 7020 have identical ARM MIDR (0x413FC090), so `dna.rs:from_cpu_id()` gave every board the same MAC `00:0A:35:AD:C0:90`. Fix: added `axi_device_dna.v` (DNA_PORT primitive → AXI registers at 0x400A_0000). `dna.rs:read_from_fpga()` now reads real 57-bit silicon DNA. Each board gets a unique MAC derived from fuse-programmed silicon ID.
26. ~~**NTC thermistor formula wrong**~~ — **FIXED (deployed March 25)**. `analog.rs` used linear approximation (ignored B coefficient entirely: `let _ = b_coeff;`), wrong divider topology (NTC on bottom vs Sonoma's NTC on top), wrong pullup (10kΩ vs 4.98kΩ). Now: proper B-equation + `ln_approx()`, Sonoma-matched divider (4980Ω pulldown + 150Ω series), default 30kΩ NTC (B=3985.3), `set_ntc_type()` for 10kΩ (B=3492.0). `read_case_temp_mc()`/`read_dut_temp_mc()` added.
27. ~~**Thermal controller not wired**~~ — **FIXED**. PID replaced with Lean-verified headroom kernel (MetabolicAge_v3.lean, Theorem 68a-e). 0 tuned constants. BU2505 DAC ch0=cooler, ch1=heater → comparator on BIM → STD16NF06LT4 FET. V×I power feedback from ADC ch24-25. Per-step temp in test plan.
28. ~~**DNA peripheral not in bitstream**~~ — **FIXED and VERIFIED March 25**. Bitstream rebuilt with `axi_device_dna.v`. Firmware reads FBC_CTRL VERSION at `0x4004_001C` (offset 0x1C, not 0x00 which is CTRL register — always 0 at reset). VERSION=0x0001_0000 confirms DNA peripheral present. Hardware MAC: `00:0A:35:C6:B4:2A` (unique per silicon DNA).

---

## AXI Register Map & Protocol

**Full register tables and 75-command protocol map:** `docs/register_map.md` + `docs/PROTOCOL.md`

Quick reference — base addresses:
- `0x4004_0000` axi_fbc_ctrl (CTRL, STATUS, CYCLE, FAST_DOUT/OEN/DIN/ERR)
- `0x4005_0000` io_config (160 pin types + pulse control)
- `0x4006_0000` axi_vector_status (ERROR_COUNT, VECTOR_COUNT, FIRST_ERR)
- `0x4007_0000` axi_freq_counter (4 channels, unused)
- `0x4008_0000` clk_wiz (Vivado IP, AXI-Lite DRP for runtime MMCM reconfiguration) — v7d April 6
- `0x4009_0000` error_bram (3× BRAMs: pattern/vector/cycle)
- `0x400A_0000` axi_device_dna (57-bit silicon ID: DNA_LO/DNA_HI/DNA_STATUS, read-only)
- `0x4040_0000` fbc_dma (MM2S: SA + LENGTH triggers transfer)

Protocol: Raw Ethernet 0x88B5, 8-byte FbcHeader (magic=0xFBC0, seq, cmd, flags, length), big-endian.

---

## GUI Command Surface

**Tauri GUI (reference):** 61 Tauri commands in `gui/src-tauri/src/lib.rs`.
Categories: Connection(3), Discovery(1), Board Control(5), Config(3), FastPins(2), Analog(1),
Power/VICOR(3), Power/PMBus(5), EEPROM(2), Vectors(6), Firmware(5), Realtime(2), Switch(4),
Export(3), Pattern Conv(1), Device Config(3), Pin Import(3), Other(5).

**Native wgpu GUI (product):** `app/` — 14 panels across 4 tabs, ~2,500 lines.
Transport layer (`app/src/transport.rs`) dispatches 37+ HwCommand variants to FbcClient or
SonomaClient. Includes 7 switch commands (serial Cisco console). See `docs/FBC.md` section N for details.

---

## Build Commands

```bash
# Firmware (bare-metal ARM)
cd firmware && cargo build --release --target armv7a-none-eabi

# GUI (Tauri + React)
cd gui && npm run tauri dev    # Development
cd gui && npm run tauri build  # Production

# FPGA Bitstream (Vivado — golden reference)
# Part: xc7z020clg484-1 (484-pin, NOT clg400!)
cd C:\Dev\projects\FBC-Semiconductor-System
vivado -mode batch -source scripts/build_bitstream.tcl
# Output: build/fbc_system.bit + reports in build/

# FPGA Toolchain (custom — separate repo)
cd C:\Dev\projects\fpga-toolchain && cargo build --release
```

### FPGA Build Notes

- **Part number**: `xc7z020clg484-1` — verified from reference/kzhang_v2_2016/kzhang_v2.xpr. Pin names like AB12 are 484-pin only.
- **Silicon revision**: IDCODE = `0x23727093` (rev 2). Scripts may reference `0x03727093` (rev 0) — both are XC7Z020, just different fab revisions.
- **PS7 IP**: Auto-generated by `scripts/build_bitstream.tcl`. Configures FCLK_CLK0=100MHz, FCLK_CLK1=200MHz, M_AXI_GP0, S_AXI_HP0, IRQ_F2P, DDR3, UART0/I2C0/SPI0/GEM0/SD0 on MIO.
- **DDR part**: MT41K256M16RE-125 assumed. If Sonoma uses different DDR, update `CONFIG.PCW_UIPARAM_DDR_PARTNO` in the TCL script.
- **Constraints**: `constraints/zynq7020_sonoma.xdc` — full 160-pin mapping from Sonoma schematics.

### JTAG Programming

- **Programmer**: FT232H breakout board via MPSSE (NOT FT2232H like Basys 3)
- **Script**: `C:\Dev\xyzt_pico_Hardware\fpga_jtag.py` — multi-device chain support for Zynq
- **J1 Header**: Molex 87832-1420, 2mm pitch, 2x7 pin. **Must solder wires** — 2.54mm dupont wires don't grip.
- **Chain**: TDI → ARM DAP (4-bit IR, 0x4BA00477) → XC7Z020 PL (6-bit IR, 0x23727093) → TDO
- **Wiring**: J1 pin 4 (TMS)→AD3, pin 6 (TCK)→AD0, pin 8 (TDO)→AD2, pin 10 (TDI)→AD1, pin 7 (GND)→GND
- **Programming**: `python fpga_jtag.py --device sonoma program build/fbc_system.bit` — 5.5s, 715 KB/s
- **Power**: 12V/3A via VCC12 pad on quad board (BK Precision 9206). Board draws ~3A with shorted FET on quad board.

---

## What the `reference/` Folder Is

The old 2016 kzhang_v2 design. Linux-based. Uses Vivado IPs, shell scripts, AWK for instrument control. **READ ONLY — for comparison, not for porting.** Our design is fundamentally different because we:
- Removed the OS (bare-metal Rust, not Linux)
- Removed Vivado (ONETWO-derived bitstream)
- Simplified the protocol (raw Ethernet, not TCP/IP stack)
- Unified the register interface (8 AXI peripherals, not scatter-gather)

### Sonoma Reference Docs (AUTHORITATIVE)

Use these paths — NOT the outdated `reference/` folder in the repo:

| Path | Contents |
|------|----------|
| `C:\Users\isaac\Downloads\sonoma_docs\` | **START HERE** — verified Sonoma hardware/firmware/vector docs |
| `C:\Users\isaac\Downloads\SOURCE\` | Sonoma source code (ELF binaries, AWK scripts) |
| `C:\Users\isaac\Downloads\Data (1)\` | Sonoma data files |

### In-Repo Reference (READ ONLY, may be outdated)

| Path | Contents |
|------|----------|
| `reference/kzhang_v2_2016/` | Original HDL source (vector.vh, io_table.v, etc.) |
| `reference/hpbicontroller-rev1/` | Altium PCB schematics |
| `reference/Everest_3.7.3_20260122_FW_v4.8C/` | Production firmware package |
| `reference/ZYNQ_REGISTER_MAP.md` | Zynq PS peripheral addresses (still valid) |

### Key Sonoma Facts

VICOR MIO: `[0, 39, 47, 8, 38, 37]` (6 pins, verified). Error formula: `oen & (dout ^ din)` (same in our io_cell.v).
Pin types 0-7 same as fbc_pkg.vh. Sonoma AXI at 0x404E0000, ours at 0x4006_0000.
Full Sonoma ELF reference + orchestration details: `SONOMA-INSTRUCTIONS.md`.

---

## What Needs Doing (Priority Order)

### Completed Milestones
- Bug fixes (1-4): FastPins, VICOR, FbcClient, rail data — all fixed
- RTL integration (5-7): DMA, error BRAMs, fast_error — all wired
- Firmware (8-10): unicast, VICOR GPIO, error BRAM readback — all working
- Bitstream: `build/fbc_system.bit` (3.9 MB) — rebuilt March 25, full timing closure WNS=+0.018ns, all 8 AXI peripherals
- First Light: ARM firmware running — March 17 (667MHz CPU, 533MHz DDR)
- AXI verified: All 8 peripherals accessible including device_dna at 0x400A_0000
- CLI: 38 FBC + 23 Sonoma commands — all tested on live hardware
- Pattern converter: .hex/.seq/.fbc generation — 166/166 tests
- March 25: All firmware fixes compiled (thermal v2, NTC, XADC u64, SD guard, DNA guard, dead code cleanup). Comms verified via TP-Link ASIX adapter. Rust compiler CRC + emit_run bugs fixed. Test vector ready (bringup_fast_pins.fbc, 102 vectors)
- March 25: 0 warnings both crates. 25/25 host tests, 11/11 firmware tests.
- March 26: DDR slot table + test plan executor fully implemented (firmware + host + CLI). 6 new FbcClient methods, 6 new CLI commands, 11 new protocol wire codes. Wire format parity verified. Cisco switch GUI integration. Native wgpu GUI at 14 panels across 4 tabs.
- **March 29: Test Plan + DDR Slots deployment ready** — firmware builds with ddr_slots.rs (280 lines) + testplan.rs (636 lines). 17 new protocol commands (0x22-0x2C, 0x35-0x36, 0xF2-0xF3). DDR checkpoint persistence at 0x0030_1000. BIM serial auto-invalidation. Power feedback loop (V×I → thermal DAC). Min/max XADC tracking. IO bank voltage control. Datalog binary format (.fbd) with CRC32.

### Strategy: CLI-First, GUI-Last

**DO NOT build or polish GUI until the CLI proves the system works end-to-end.**

The CLI is the verification layer. Every command, every response, byte-for-byte on real hardware.
The GUI is just a skin on verified foundations — it gets built AFTER the system is proven.

```
Layer 0: RTL + Firmware (FPGA + ARM)     ← DONE — bitstream rebuilt, all 8 AXI peripherals, timing closure
Layer 0.5: BOOT.BIN                      ← DONE — deployed via JTAG March 25
Layer 1: CLI (host crate)                ← DONE — 38 FBC + 23 Sonoma commands, DDR slots + test plan executor
Layer 1.5: Autonomous burn-in            ← CURRENT — first vector run on hardware (needs 48V + tray)
Layer 2: C Engine (pattern converter)    ← DONE, 166/166 tests
Layer 3: GUI (wgpu native app/)          ← IN PROGRESS — 14 panels, 4 tabs, switch integration, dual-profile transport
```

**Why:** You don't paint the dashboard before the engine runs. If the CLI can
discover → configure → upload → run → collect errors → export, then the GUI
is just invoke() calls to the same proven functions. If the CLI can't do it,
no amount of React components will fix it.

### What's Next (Priority Order)

18. ~~**Rebuild bitstream**~~ — **DONE March 25**. WNS=+0.018ns, all 8 AXI peripherals. DNA MAC verified: `00:0A:35:C6:B4:2A`.
19. ~~**Package BOOT.BIN**~~ — **DONE March 25**. Deployed via JTAG.
20. ~~**First vector run**~~ — **DONE March 29**. Test Plan + DDR Slots implementation complete.
21. ~~**Verify XADC temp**~~ — **DONE March 25**. Reads 39.5°C (confirms u64 fix).
22. ~~**CLI: Full burn-in flow**~~ — **DONE March 29**. `slot-upload → set-plan → run-plan → plan-status` implemented in firmware + host + CLI. Needs 48V + tray for hardware verification.
23. **CLI: Sonoma burn-in flow** — same sequence via SSH, verified against production ELFs
24. **CLI: Multi-board orchestration** — discover N boards, run same test on all, collect all results
25. ~~Fix clk_ctrl AXI crash~~ — **DONE March 25**. Root cause: incomplete `case` in AXI write FSM. Verified on hardware.
26. ~~Thermal GPIO routing~~ — **DONE March 27**. NOT GPIO — heater/cooler via BU2505 DAC (ch1=heater, ch0=cooler). DAC→comparator→FET on BIM. Wired in main.rs safety loop.
27. **GUI integration** — wire verified CLI commands to native wgpu app panels (transport.rs already has FBC+Sonoma dispatch)
28. ~~Clean up dead code (TcpServer, UdpPacket in net.rs)~~ — **DONE March 25** (268 lines deleted from firmware).
29. ~~**DDR slot table + test plan executor**~~ — **DONE March 26, redesigned March 29**. SD pattern storage (256 max) + DDR double-buffer (A/B regions) replaces fixed 8-slot model. PlanExecutor with `pattern_id` references, per-step temp+clock, checkpoint persistence. MAX_STEPS bumped to 96 (real projects: up to 91).
30. ~~**Cisco switch GUI integration**~~ — **DONE March 26**. Serial console in Facility→Network tab, MAC→board cross-reference, VLAN reconfiguration.
31. ~~**Firmware FEATURE-COMPLETE (March 28)**~~ — **DONE March 29**. All Sonoma gaps closed. Compiles clean (43 tests, 0 warnings). DDR slots + test plan executor + thermal DAC + V×I feedback + undervoltage + per-step temp+clock + IO bank + min/max XADC + network health.
32. ~~**Hardware validation (March 30)**~~ — **12/12 commands verified on hardware**. XADC reads correct (VCCINT=1001mV, VCCAUX=1795mV, 41°C die temp). Analog first-read timeout fixed (SPI 10ms→100µs). Safety monitor runs continuously.
33. ~~**clk_wiz replacement (April 6)**~~ — **DONE**. Hand-rolled clk_ctrl replaced with Vivado clk_wiz IP. v7d bitstream WNS=+0.028ns. Firmware uses DRP registers. Pending hardware clock switching test.
34. **April updates:** Headroom thermal kernel (Lean-verified, replaces PID), VICOR current readback, gen-bim CLI command, AVC/STIL parser fixes, ADC CS2 fix, XADC scaling fix (÷65536), repl.rs registered, datalog .fbd format.
35. **Next: 48V power-on test** — connect 48V, verify PMBus/VICOR/ADC with real supplies. Program EEPROM. Run first vectors.

### What the GUI is NOT

The GUI (`app/`, native wgpu) is a thin layer over verified CLI commands. The Tauri GUI (`gui/`)
is reference only — the native app is the product. The real validation path is:

```
CLI command works on hardware  →  HwCommand variant in transport.rs  →  Panel calls send_command()
```

Any GUI work that doesn't follow this chain is premature. Count CLI commands verified on hardware,
not panel count. The native app has 14 panels across 4 tabs (Dashboard, Profiling, Engineering, Datalogs),
dual-profile transport (FBC + Sonoma), and Cisco switch integration — but correctness flows from the CLI.

---

## Uncertainties (Need Hardware/Schematic Verification)

These values are in the code but have NOT been verified against the actual hardware:

| What | Value in Code | File | Risk |
|------|--------------|------|------|
| VICOR DAC multiplier | 2× | `vicor.rs:36` | **VERIFIED** — Sonoma uses `voltage*2` in linux_VICOR_Voltage.elf |
| XADC voltage scale | 3000mV | `analog.rs:64` | **VERIFIED** — ADR5043BKSZ 3.0V precision reference on PCB |
| PMBus I2C addresses | `lcps_channel_to_addr()` | `hal/pmbus.rs` | May vary across boards |
| BIM EEPROM format | `BimEeprom` struct | `hal/eeprom.rs` | No external spec, only code |
| VICOR enable MIO pins | `[0, 39, 47, 8, 38, 37]` | `main.rs:94` (SLCR configured) | **VERIFIED.** 6 pins including MIO0 (Core 1). Matches Sonoma VICOR_MAP. |
| MIO 36 = ADC bank select | ToggleMio 36 for ch 16-31 | Sonoma ReadAnalog.awk | Must not conflict with other MIO 36 usage |
| Clock freq boundaries | 7.5/17.5/37.5/75 MHz | `regs.rs:393-401` | Edge-case freq may pick wrong preset |
| ERR_TRIG pin type (0x6) | Falls to BIDI | `io_cell.v:244` | Marked "causes timing problems" |
| Flight recorder capacity | 1000 sectors (100s) | `main.rs:426` | May need more for long tests |
