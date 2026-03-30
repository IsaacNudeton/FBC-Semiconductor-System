# Pattern Converter Migration Guide

**Purpose:** Document the path from legacy ATP/STIL/AVC patterns to FBC compressed format.

**Date:** March 2026
**Status:** ✅ **COMPLETE — gen_fbc.c integrated**

---

## The Problem (FIXED March 2026)

~~Customer patterns in ATP/STIL/AVC format **cannot be converted to `.fbc`** (compressed FBC format).~~

```
ATP/STIL/AVC (customer patterns)
    │
    ▼
gui/src-tauri/c-engine/pc/  ✅ COMPLETE (14 C files)
    │
    ├──▶ .hex + .seq        ✅ WORKS (40 bytes/vector, uncompressed)
    │
    └──▶ .fbc               ✅ WORKS (1-21 bytes/vector, compressed)
                              ▲
                              │
.fvec ──▶ host/src/vector/  ✅ COMPLETE (Rust compiler)
```

---

## Why This Matters

### Format Comparison

| Format | Size per Vector | Compression | Used By |
|--------|----------------|-------------|---------|
| **`.hex`** | 40 bytes (fixed) | 1x (none) | Legacy Sonoma (Linux/kzhang_v2) |
| **`.fbc`** | 1-21 bytes (variable) | 4.8-710x | FBC System (bare-metal Rust) |

### `.fbc` Opcodes

| Opcode | Size | Use Case |
|--------|------|----------|
| `VECTOR_ZERO` (0x04) | 1 byte | All pins low |
| `VECTOR_ONES` (0x05) | 1 byte | All pins high |
| `VECTOR_RUN` (0x03) | 5 bytes | Repeat previous vector N times |
| `VECTOR_SPARSE` (0x02) | 2+N bytes | Small changes from previous (≤15 toggles) |
| `VECTOR_FULL` (0x01) | 21 bytes | Complete 160-bit vector |
| `VECTOR_XOR` (0x06) | 21 bytes | XOR with previous vector |

### Real Compression Numbers (Verified)

| File | Vectors | Uncompressed | Compressed | Ratio |
|------|---------|--------------|------------|-------|
| `test_core.fbc` | 2,759,718 | 55.2 MB | 77.7 KB | **710x** |
| `test_stil.fbc` | 18,539 | 371 KB | 77 KB | **4.8x** |

**Verified command:**
```bash
C:\Dev\projects\FBC-Semiconductor-System\host\target\release\fbc-vec.exe info reference/scratch/test_core.fbc
```

---

## What Exists Today

### ✅ Complete: C Engine Pattern Converter

**Location:** `gui/src-tauri/c-engine/pc/` (14 C files)

| File | Purpose | Lines |
|------|---------|-------|
| `pc.h` | Header — all types, API | 263 |
| `dll_api.c` | FFI API (handles) | 134 |
| `ir.c` | IR lifecycle, encoding | 134 |
| `crc32.c` | CRC32 checksum | 33 |
| `parse_atp.c` | ATP parser | 276 |
| `parse_stil_smart.c` | STIL smart parser | 403 |
| `parse_avc_smart.c` | AVC smart parser | 278 |
| `parse_pinmap.c` | Pin map parser (3 formats) | 154 |
| `gen_hex.c` | `.hex` generator (40B/vector) | 99 |
| `gen_seq.c` | `.seq` generator | 33 |
| `dc.h` | Device config header | 216 |
| `dc_api.c` | Device config FFI API | 193 |
| `dc_json.c` | JSON parser | (included) |
| `dc_gen.c` | Config file generators | (included) |
| `vendor/cJSON.c` | JSON library | (included) |

**Tauri Commands:**
- `pc_convert` — ATP/STIL/AVC → `.hex` + `.seq`
- `dc_generate_config` — Device JSON → PIN_MAP + .map + .lvl + .tim + .tp + PowerOn/Off.sh
- `dc_generate_file` — Generate single config file type
- `pc_version` — Version info

**GUI:** `PatternConverterPanel.tsx` (1019 lines, 3 tabs)

---

### ✅ Complete: Rust FBC Compiler

**Location:** `host/src/vector/`

| File | Purpose |
|------|---------|
| `format.rs` | `.fbc` binary format (header, opcodes, read/write) |
| `fvec.rs` | `.fvec` text format parser |
| `compiler.rs` | Compiles `.fvec` → `.fbc` (compressed) |

**CLI:** `fbc-vec compile input.fvec -o output.fbc`

**Compression Strategy:**
```rust
if vector == all_zeros      → VECTOR_ZERO (1 byte)
else if vector == all_ones   → VECTOR_ONES (1 byte)
else if vector == previous   → VECTOR_RUN (accumulate count)
else if hamming(vector, prev) <= 15 → VECTOR_SPARSE (2+N bytes)
else                           → VECTOR_FULL (21 bytes)
```

---

### ✅ Complete: ATP/STIL/AVC → `.fbc` (March 2026)

**Implemented as Option 1** — `gen_fbc.c` added to C engine (517 lines).

```c
// gui/src-tauri/c-engine/pc/gen_fbc.c
int pc_gen_fbc(const PcPattern *p, const char *path, uint32_t vec_clock_hz);
```

- Full FBC header generation (magic 0x00434246, version, pin count, CRC32)
- All compression opcodes: VECTOR_ZERO, VECTOR_ONES, VECTOR_RUN, VECTOR_SPARSE, VECTOR_FULL, VECTOR_XOR
- Byte-compatible with Rust compiler (`host/src/vector/compiler.rs`)
- DLL API: `pc_dll_gen_fbc(handle, path, vec_clock_hz)`
- Tauri command: `pc_convert` with `fbc_output` parameter
- Frontend: `.fbc OUT` file picker in Pattern Conversion tab

**Verification:** Compression ratios match Rust compiler (4.8x-710x).

---

## Related Files

| File | Purpose |
|------|---------|
| `gui/src-tauri/c-engine/pc/pc.h` | C engine header |
| `gui/src-tauri/c-engine/pc/gen_fbc.c` | `.fbc` generator (517 lines) |
| `gui/src-tauri/c-engine/pc/gen_hex.c` | `.hex` generator (reference) |
| `host/src/vector/format.rs` | `.fbc` format spec |
| `host/src/vector/compiler.rs` | Compression logic (Rust equivalent) |
| `firmware/src/fbc_decompress.rs` | `.fbc` decompressor (firmware side) |
| `rtl/fbc_decoder.v` | FPGA bytecode decoder |
