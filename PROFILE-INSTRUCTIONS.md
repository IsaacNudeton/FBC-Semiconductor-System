# Built-in Tester Profile Implementation Guide

**For:** Qwen (or any AI continuing this work)
**Task:** Add HX, XP-160/Shasta, and MCC built-in profiles to the C pattern converter engine
**Priority:** High — these are production systems at ISE Labs
**Owner:** Isaac Oravec / ISE Labs

---

## Project Overview

**FBC-Semiconductor-System** (`C:\Dev\projects\FBC-Semiconductor-System`) is the ONE unified GUI
application for controlling ALL burn-in tester systems at ISE Labs. It replaces multiple legacy
tools with a single Tauri 2 + React + Rust + C app.

### Architecture Stack

```
┌─────────────────────────────────────────────────────────────────┐
│  FRONTEND — React + TypeScript                                   │
│  gui/src/components/PatternConverterPanel.tsx  (main UI)         │
│  gui/src/components/PatternConverterPanel.css  (styles)          │
│  gui/src/App.tsx                               (router)          │
│  gui/src/components/Sidebar.tsx                (navigation)      │
├─────────────────────────────────────────────────────────────────┤
│  TAURI BRIDGE — Rust commands in gui/src-tauri/src/              │
│  gui/src-tauri/src/lib.rs          (all Tauri commands, ~1600L)  │
│  gui/src-tauri/src/pattern_converter/pc_ffi.rs (Rust→C FFI)     │
│  gui/src-tauri/src/pattern_converter/mod.rs    (module exports)  │
│  gui/src-tauri/src/pattern_converter/pin_extractor.rs (Pin Import) │
│  gui/src-tauri/Cargo.toml          (Rust dependencies)           │
├─────────────────────────────────────────────────────────────────┤
│  C ENGINE — Zero-dependency C library, compiled by build.rs      │
│  gui/src-tauri/c-engine/pc/pc.h    (pattern converter header)    │
│  gui/src-tauri/c-engine/pc/dc.h    (device config header)        │
│  gui/src-tauri/c-engine/pc/dc_json.c (profile parser + built-ins)│
│  gui/src-tauri/c-engine/pc/dc_gen.c  (file generators)           │
│  gui/src-tauri/c-engine/pc/dc_csv.c  (CSV parser)                │
│  gui/src-tauri/c-engine/pc/dc_api.c  (DLL/FFI entry points)      │
│  gui/src-tauri/c-engine/pc/gen_hex.c (legacy .hex output)        │
│  gui/src-tauri/c-engine/pc/gen_seq.c (.seq output)               │
│  gui/src-tauri/c-engine/pc/gen_fbc.c (compressed .fbc output)    │
│  gui/src-tauri/c-engine/pc/dll_api.c (pattern converter FFI)     │
│  gui/src-tauri/build.rs            (compiles C via cc crate)      │
├─────────────────────────────────────────────────────────────────┤
│  REFERENCE — FSHC project has tester definitions                 │
│  C:\Dev\FSHC - Hardware\fshc\crates\fshc-platform\src\tester.rs │
│  (Complete Rust TesterProfile structs for all 5 systems)         │
└─────────────────────────────────────────────────────────────────┘
```

### How Profiles Flow Through the System

```
User picks profile in GUI dropdown (PatternConverterPanel.tsx)
    │
    ▼  invoke('dc_generate_config', { profile: "sonoma", ... })
Tauri command in lib.rs
    │
    ▼  DeviceConfigGenerator::load_profile("sonoma")
Rust FFI wrapper (pc_ffi.rs)
    │
    ▼  dc_load_profile(handle, "sonoma")
C DLL API (dc_api.c)
    │
    ▼  dc_get_builtin_profile("sonoma")  →  returns JSON string
C Profile Parser (dc_json.c)
    │
    ▼  dc_parse_profile(json, &profile)  →  fills DcTesterProfile struct
C Generators (dc_gen.c)
    │
    ▼  dc_gen_all(&profile, &device, output_dir)
Output: PIN_MAP + .map + .lvl + .tim + .tp + PowerOn.sh + PowerOff.sh
```

### The 4 Production Tester Systems

ISE Labs operates these burn-in test systems:

| System | Controller | Channels | Axes | Supplies | Status in App |
|--------|-----------|----------|------|----------|--------------|
| **Sonoma** | Zynq 7020 (Aehr) | 128 | 1 | 6 VICOR | **COMPLETE** |
| **HX** | XPS-4 (Aehr/Incal) | 160/axis | 4 | 16 (RMA5608) | **MISSING** |
| **XP-160/Shasta** | XPS-8 (Aehr/Incal) | 160/axis | 8 | 32 (RMA5608) | **MISSING** |
| **MCC** | Custom (ISE Labs) | 128 | 1 | 8 | **MISSING** |

**Key insight:** HX and XP-160/Shasta use the **same driver** — Shasta is just the newer
version of XP-160. The only real difference is axis count (4 vs 8). Per-axis layout is
identical: 96 drive + 60 monitor + 4 reserved = 160 channels.

---

## Files to Modify (with full paths)

### File 1: `C:\Dev\projects\FBC-Semiconductor-System\gui\src-tauri\c-engine\pc\dc.h`

**What it is:** Header defining all device config structs — `DcTesterProfile`, `DcDeviceIR`,
`DcGpioBank`, `DcCoreHw`, `DcSupplyAssign`, `DcTestStep`, etc.

**Change needed:** Bump `DC_MAX_SUPPLIES` from 16 to 32 (XP-160 has 32 power supplies).

```c
// Line 57, change:
#define DC_MAX_SUPPLIES  16
// To:
#define DC_MAX_SUPPLIES  32
```

Optional: Add `pattern_zones` field to `DcTesterProfile` for MCC support:
```c
typedef struct {
    char       name[DC_MAX_NAME];
    int        total_channels;
    DcGpioBank banks[DC_MAX_BANKS];
    int        num_banks;
    DcCoreHw   cores[DC_MAX_SUPPLIES];
    int        num_cores;
    char       firmware_path[DC_MAX_NAME];
    char       vector_dir[DC_MAX_NAME];
    double     default_period_ns;
    double     default_drive_on_ns;
    double     default_drive_off_ns;
    double     default_compare_ns;
    int        pattern_zones;           /* NEW — 0 for most, 16 for MCC */
} DcTesterProfile;
```

---

### File 2: `C:\Dev\projects\FBC-Semiconductor-System\gui\src-tauri\c-engine\pc\dc_json.c`

**What it is:** JSON parser for tester profiles + device configs. Contains the built-in
Sonoma profile as a C string literal. Uses vendored cJSON for parsing.

**Current state:** Only `SONOMA_PROFILE_JSON` exists (lines 18-41). The lookup function
`dc_get_builtin_profile()` only matches "sonoma" (line 202-208).

**Change 1:** After `SONOMA_PROFILE_JSON` (after line 41), add three new profile strings:

```c
/* ═══════════════════════════════════════════════════════════════
 * BUILT-IN HX PROFILE
 *
 * Aehr Test Systems (Incal heritage), XPS-4 controller
 * 4 axes × 160 channels = 640 total per system
 * Each axis: 96 drive + 60 monitor + 4 reserved
 * RMA5608 Power Train, INSPIRE v4.9 software
 * ═══════════════════════════════════════════════════════════════ */

static const char HX_PROFILE_JSON[] =
"{"
"  \"name\": \"HX\","
"  \"total_channels\": 160,"
"  \"banks\": ["
"    {\"name\": \"DRIVE\",   \"start_pin\": 0,   \"num_pins\": 96},"
"    {\"name\": \"MONITOR\", \"start_pin\": 96,  \"num_pins\": 60},"
"    {\"name\": \"RESERVED\",\"start_pin\": 156, \"num_pins\": 4}"
"  ],"
"  \"cores\": ["
"    {\"name\": \"PS1\",  \"dac_channel\": 0, \"mio_pin\": 0, \"default_voltage\": 0.0},"
"    {\"name\": \"PS2\",  \"dac_channel\": 1, \"mio_pin\": 1, \"default_voltage\": 0.0},"
"    {\"name\": \"PS3\",  \"dac_channel\": 2, \"mio_pin\": 2, \"default_voltage\": 0.0},"
"    {\"name\": \"PS4\",  \"dac_channel\": 3, \"mio_pin\": 3, \"default_voltage\": 0.0},"
"    {\"name\": \"PS5\",  \"dac_channel\": 4, \"mio_pin\": 4, \"default_voltage\": 0.0},"
"    {\"name\": \"PS6\",  \"dac_channel\": 5, \"mio_pin\": 5, \"default_voltage\": 0.0},"
"    {\"name\": \"PS7\",  \"dac_channel\": 6, \"mio_pin\": 6, \"default_voltage\": 0.0},"
"    {\"name\": \"PS8\",  \"dac_channel\": 7, \"mio_pin\": 7, \"default_voltage\": 0.0},"
"    {\"name\": \"PS9\",  \"dac_channel\": 8, \"mio_pin\": 8, \"default_voltage\": 0.0},"
"    {\"name\": \"PS10\", \"dac_channel\": 9, \"mio_pin\": 9, \"default_voltage\": 0.0},"
"    {\"name\": \"PS11\", \"dac_channel\": 10,\"mio_pin\": 10,\"default_voltage\": 0.0},"
"    {\"name\": \"PS12\", \"dac_channel\": 11,\"mio_pin\": 11,\"default_voltage\": 0.0},"
"    {\"name\": \"PS13\", \"dac_channel\": 12,\"mio_pin\": 12,\"default_voltage\": 0.0},"
"    {\"name\": \"PS14\", \"dac_channel\": 13,\"mio_pin\": 13,\"default_voltage\": 0.0},"
"    {\"name\": \"PS15\", \"dac_channel\": 14,\"mio_pin\": 14,\"default_voltage\": 0.0},"
"    {\"name\": \"PS16\", \"dac_channel\": 15,\"mio_pin\": 15,\"default_voltage\": 0.0}"
"  ],"
"  \"firmware_path\": \"\","
"  \"vector_dir\": \"\","
"  \"default_period_ns\": 200.0,"
"  \"default_drive_on_ns\": 0.0,"
"  \"default_drive_off_ns\": 90.0,"
"  \"default_compare_ns\": 100.0"
"}";

/* ═══════════════════════════════════════════════════════════════
 * BUILT-IN XP-160 / SHASTA PROFILE
 *
 * Aehr Test Systems (Incal heritage), XPS-8 controller
 * 8 axes × 160 channels = 1280 total per system
 * Each axis: 96 drive + 60 monitor + 4 reserved
 * RMA5608 Power Train, INSPIRE XP8 v1.3.16 software
 * Same driver as HX — Shasta is just the newer version of XP-160
 * ═══════════════════════════════════════════════════════════════ */

static const char XP160_PROFILE_JSON[] =
"{"
"  \"name\": \"XP-160/Shasta\","
"  \"total_channels\": 160,"
"  \"banks\": ["
"    {\"name\": \"DRIVE\",   \"start_pin\": 0,   \"num_pins\": 96},"
"    {\"name\": \"MONITOR\", \"start_pin\": 96,  \"num_pins\": 60},"
"    {\"name\": \"RESERVED\",\"start_pin\": 156, \"num_pins\": 4}"
"  ],"
"  \"cores\": ["
"    {\"name\": \"PS1\",  \"dac_channel\": 0, \"mio_pin\": 0, \"default_voltage\": 0.0},"
"    {\"name\": \"PS2\",  \"dac_channel\": 1, \"mio_pin\": 1, \"default_voltage\": 0.0},"
"    {\"name\": \"PS3\",  \"dac_channel\": 2, \"mio_pin\": 2, \"default_voltage\": 0.0},"
"    {\"name\": \"PS4\",  \"dac_channel\": 3, \"mio_pin\": 3, \"default_voltage\": 0.0},"
"    {\"name\": \"PS5\",  \"dac_channel\": 4, \"mio_pin\": 4, \"default_voltage\": 0.0},"
"    {\"name\": \"PS6\",  \"dac_channel\": 5, \"mio_pin\": 5, \"default_voltage\": 0.0},"
"    {\"name\": \"PS7\",  \"dac_channel\": 6, \"mio_pin\": 6, \"default_voltage\": 0.0},"
"    {\"name\": \"PS8\",  \"dac_channel\": 7, \"mio_pin\": 7, \"default_voltage\": 0.0},"
"    {\"name\": \"PS9\",  \"dac_channel\": 8, \"mio_pin\": 8, \"default_voltage\": 0.0},"
"    {\"name\": \"PS10\", \"dac_channel\": 9, \"mio_pin\": 9, \"default_voltage\": 0.0},"
"    {\"name\": \"PS11\", \"dac_channel\": 10,\"mio_pin\": 10,\"default_voltage\": 0.0},"
"    {\"name\": \"PS12\", \"dac_channel\": 11,\"mio_pin\": 11,\"default_voltage\": 0.0},"
"    {\"name\": \"PS13\", \"dac_channel\": 12,\"mio_pin\": 12,\"default_voltage\": 0.0},"
"    {\"name\": \"PS14\", \"dac_channel\": 13,\"mio_pin\": 13,\"default_voltage\": 0.0},"
"    {\"name\": \"PS15\", \"dac_channel\": 14,\"mio_pin\": 14,\"default_voltage\": 0.0},"
"    {\"name\": \"PS16\", \"dac_channel\": 15,\"mio_pin\": 15,\"default_voltage\": 0.0},"
"    {\"name\": \"PS17\", \"dac_channel\": 16,\"mio_pin\": 16,\"default_voltage\": 0.0},"
"    {\"name\": \"PS18\", \"dac_channel\": 17,\"mio_pin\": 17,\"default_voltage\": 0.0},"
"    {\"name\": \"PS19\", \"dac_channel\": 18,\"mio_pin\": 18,\"default_voltage\": 0.0},"
"    {\"name\": \"PS20\", \"dac_channel\": 19,\"mio_pin\": 19,\"default_voltage\": 0.0},"
"    {\"name\": \"PS21\", \"dac_channel\": 20,\"mio_pin\": 20,\"default_voltage\": 0.0},"
"    {\"name\": \"PS22\", \"dac_channel\": 21,\"mio_pin\": 21,\"default_voltage\": 0.0},"
"    {\"name\": \"PS23\", \"dac_channel\": 22,\"mio_pin\": 22,\"default_voltage\": 0.0},"
"    {\"name\": \"PS24\", \"dac_channel\": 23,\"mio_pin\": 23,\"default_voltage\": 0.0},"
"    {\"name\": \"PS25\", \"dac_channel\": 24,\"mio_pin\": 24,\"default_voltage\": 0.0},"
"    {\"name\": \"PS26\", \"dac_channel\": 25,\"mio_pin\": 25,\"default_voltage\": 0.0},"
"    {\"name\": \"PS27\", \"dac_channel\": 26,\"mio_pin\": 26,\"default_voltage\": 0.0},"
"    {\"name\": \"PS28\", \"dac_channel\": 27,\"mio_pin\": 27,\"default_voltage\": 0.0},"
"    {\"name\": \"PS29\", \"dac_channel\": 28,\"mio_pin\": 28,\"default_voltage\": 0.0},"
"    {\"name\": \"PS30\", \"dac_channel\": 29,\"mio_pin\": 29,\"default_voltage\": 0.0},"
"    {\"name\": \"PS31\", \"dac_channel\": 30,\"mio_pin\": 30,\"default_voltage\": 0.0},"
"    {\"name\": \"PS32\", \"dac_channel\": 31,\"mio_pin\": 31,\"default_voltage\": 0.0}"
"  ],"
"  \"firmware_path\": \"\","
"  \"vector_dir\": \"\","
"  \"default_period_ns\": 200.0,"
"  \"default_drive_on_ns\": 0.0,"
"  \"default_drive_off_ns\": 90.0,"
"  \"default_compare_ns\": 100.0"
"}";

/* ═══════════════════════════════════════════════════════════════
 * BUILT-IN MCC PROFILE
 *
 * ISE Labs custom burn-in system
 * 128 channels, 8 power supplies
 * Watlow thermal via Modbus TCP/IP, PLC integration
 * 16 configurable pattern zones (unique to MCC)
 * Coarser timing than Incal systems (1ns vs 200ps)
 * ═══════════════════════════════════════════════════════════════ */

static const char MCC_PROFILE_JSON[] =
"{"
"  \"name\": \"MCC\","
"  \"total_channels\": 128,"
"  \"banks\": ["
"    {\"name\": \"BANK0\", \"start_pin\": 0,   \"num_pins\": 32},"
"    {\"name\": \"BANK1\", \"start_pin\": 32,  \"num_pins\": 32},"
"    {\"name\": \"BANK2\", \"start_pin\": 64,  \"num_pins\": 32},"
"    {\"name\": \"BANK3\", \"start_pin\": 96,  \"num_pins\": 32}"
"  ],"
"  \"cores\": ["
"    {\"name\": \"PS1\", \"dac_channel\": 0, \"mio_pin\": 0, \"default_voltage\": 0.0},"
"    {\"name\": \"PS2\", \"dac_channel\": 1, \"mio_pin\": 1, \"default_voltage\": 0.0},"
"    {\"name\": \"PS3\", \"dac_channel\": 2, \"mio_pin\": 2, \"default_voltage\": 0.0},"
"    {\"name\": \"PS4\", \"dac_channel\": 3, \"mio_pin\": 3, \"default_voltage\": 0.0},"
"    {\"name\": \"PS5\", \"dac_channel\": 4, \"mio_pin\": 4, \"default_voltage\": 0.0},"
"    {\"name\": \"PS6\", \"dac_channel\": 5, \"mio_pin\": 5, \"default_voltage\": 0.0},"
"    {\"name\": \"PS7\", \"dac_channel\": 6, \"mio_pin\": 6, \"default_voltage\": 0.0},"
"    {\"name\": \"PS8\", \"dac_channel\": 7, \"mio_pin\": 7, \"default_voltage\": 0.0}"
"  ],"
"  \"firmware_path\": \"\","
"  \"vector_dir\": \"\","
"  \"default_period_ns\": 1000.0,"
"  \"default_drive_on_ns\": 0.0,"
"  \"default_drive_off_ns\": 450.0,"
"  \"default_compare_ns\": 500.0"
"}";
```

**Change 2:** Update `dc_get_builtin_profile()` (around line 202):

Current:
```c
const char *dc_get_builtin_profile(const char *name)
{
    if (!name) return NULL;
    if (strcasecmp(name, "sonoma") == 0)
        return SONOMA_PROFILE_JSON;
    return NULL;
}
```

Replace with:
```c
const char *dc_get_builtin_profile(const char *name)
{
    if (!name) return NULL;
    if (strcasecmp(name, "sonoma") == 0)
        return SONOMA_PROFILE_JSON;
    if (strcasecmp(name, "hx") == 0)
        return HX_PROFILE_JSON;
    if (strcasecmp(name, "xp160") == 0 || strcasecmp(name, "xp-160") == 0 ||
        strcasecmp(name, "shasta") == 0)
        return XP160_PROFILE_JSON;
    if (strcasecmp(name, "mcc") == 0)
        return MCC_PROFILE_JSON;
    return NULL;
}
```

---

### File 3: `C:\Dev\projects\FBC-Semiconductor-System\gui\src\components\PatternConverterPanel.tsx`

**What it is:** The main React component for the Pattern Converter panel. Has 3 tabs:
Pattern Conversion, Device Config, and Pin Import. Each tab has a profile dropdown.

**Current state:** All dropdowns only show Sonoma. There are **3 separate dropdowns**
using 2 different state variables (`profile` for Device Config, `importProfile` for Pin Import).
The Pattern Conversion tab doesn't have a profile dropdown (it doesn't need one — it generates
.hex/.seq/.fbc which are format-agnostic).

**Change:** Find ALL `<select>` elements with profile options and add the new systems.

**Device Config tab dropdown** (around line 593):
```tsx
// BEFORE:
<select value={profile} onChange={(e) => setProfile(e.target.value)}>
  <option value="sonoma">Sonoma (built-in)</option>
</select>

// AFTER:
<select value={profile} onChange={(e) => setProfile(e.target.value)}>
  <option value="sonoma">Sonoma — 128ch, 6 cores, Zynq 7020</option>
  <option value="hx">HX — 160ch/axis × 4 axes, 16 supplies, Incal</option>
  <option value="xp160">XP-160/Shasta — 160ch/axis × 8 axes, 32 supplies, Incal</option>
  <option value="mcc">MCC — 128ch, 8 supplies, pattern zones, ISE Labs</option>
</select>
```

**Pin Import tab dropdown** (around line 752):
```tsx
// BEFORE:
<select value={importProfile} onChange={(e) => setImportProfile(e.target.value)}>
  <option value="sonoma">Sonoma (built-in)</option>
</select>

// AFTER:
<select value={importProfile} onChange={(e) => setImportProfile(e.target.value)}>
  <option value="sonoma">Sonoma — 128ch, 6 cores, Zynq 7020</option>
  <option value="hx">HX — 160ch/axis × 4 axes, 16 supplies, Incal</option>
  <option value="xp160">XP-160/Shasta — 160ch/axis × 8 axes, 32 supplies, Incal</option>
  <option value="mcc">MCC — 128ch, 8 supplies, pattern zones, ISE Labs</option>
</select>
```

---

## NO changes needed to these files

These files do NOT need modification — they already handle arbitrary profile names:

| File | Why it's fine |
|------|--------------|
| `dc_api.c` | `dc_load_profile()` calls `dc_get_builtin_profile()` — adding profiles there is enough |
| `dc_gen.c` | Generators work from `DcTesterProfile` struct — profile-agnostic |
| `dc_csv.c` | CSV parser fills `DcDeviceIR` — independent of profile |
| `pc_ffi.rs` | Rust FFI passes profile name as string — no enum to update |
| `lib.rs` | Tauri command passes profile string directly to C — no filtering |
| `build.rs` | No new .c files — just editing existing dc_json.c |
| `Cargo.toml` | No new Rust dependencies |

---

## System Comparison (for reference)

| Property | Sonoma | HX | XP-160/Shasta | MCC |
|----------|--------|----|---------------|-----|
| **Vendor** | Aehr | Aehr (Incal) | Aehr (Incal) | ISE Labs |
| **Channels/axis** | 128 | 160 | 160 | 128 |
| **Axes** | 1 | 4 | 8 | 1 |
| **Total channels** | 128 | 640 | 1280 | 128 |
| **Power supplies** | 6 (VICOR) | 16 (RMA5608) | 32 (RMA5608) | 8 |
| **Timing resolution** | 100ps | 200ps | 200ps | 1000ps |
| **Max rate** | 200MHz | 200MHz | 200MHz | 50MHz |
| **Thermal** | Watlow, 4 zones | RMA5608, 4 zones | RMA5608, 8 zones | Watlow, 1 zone |
| **Software** | HPBI Controller | INSPIRE v4.9 | INSPIRE XP8 | MCC Controller |
| **Pattern tool** | VelocityCAE | PatConvert.exe | PatConvert.exe | (custom) |
| **Vector formats** | STIL/APS/AVC | STIL/AVC/APS | STIL/AVC/APS | APS/Binary |
| **Pattern memory** | DMA from ARM | 650K vectors | 650K vectors/axis | 16 zones |

**HX and XP-160 are the same driver** — XP-160/Shasta just has more axes (8 vs 4).
The per-axis channel layout is identical: 96 drive + 60 monitor + 4 reserved = 160.

---

## Important Notes

- **DC_MAX_SUPPLIES is 16** in `dc.h` — XP-160 has 32 supplies, so you MUST bump
  `DC_MAX_SUPPLIES` to 32 (or 64 for safety). This is a simple `#define` change in
  `gui/src-tauri/c-engine/pc/dc.h` line 57.
- The `total_channels` in the profile is **per axis** (160 for HX/XP-160). The GUI/docs
  should clarify that HX = 4 × 160 and XP-160 = 8 × 160 total.
- MCC has pattern zones (16) — the current `DcTesterProfile` struct doesn't have a
  `pattern_zones` field. Consider adding it to `dc.h` if MCC zone support is needed.
- Power supply names (PS1-PS32) are placeholders. Update with real names when Isaac
  provides hardware documentation from the actual boards.
- `firmware_path` and `vector_dir` are empty for HX/XP-160/MCC — these will be filled
  in once we have the actual hardware and know the file system layout.

---

## Verification

After making changes:
```bash
cd C:\Dev\projects\FBC-Semiconductor-System\gui\src-tauri
cargo check  # Should compile with zero new errors
```

Test that all profiles load:
- The existing test infrastructure in the C engine should be extended
- At minimum, verify `dc_get_builtin_profile("hx")` returns non-NULL
- Verify `dc_get_builtin_profile("shasta")` returns the XP-160 profile (alias)
- Verify `dc_get_builtin_profile("xp-160")` also returns the XP-160 profile (hyphenated alias)

---

## Source of Truth

- **FSHC tester.rs:** `C:\Dev\FSHC - Hardware\fshc\crates\fshc-platform\src\tester.rs`
  - Lines 210-259: Sonoma profile (Rust)
  - Lines 263-309: MCC profile (Rust)
  - Lines 314-367: XP-160/Shasta profile + shasta alias (Rust)
  - Lines 372-422: HX profile (Rust)
- **Existing Sonoma C profile:** `gui/src-tauri/c-engine/pc/dc_json.c` lines 18-41
- **DC structs:** `gui/src-tauri/c-engine/pc/dc.h` (DcTesterProfile, DcGpioBank, DcCoreHw)
- **This document:** `C:\Dev\projects\FBC-Semiconductor-System\PROFILE-INSTRUCTIONS.md`
