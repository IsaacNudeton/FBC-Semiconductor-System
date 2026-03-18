# Documentation Audit & Consolidation Plan

**Date:** March 13, 2026  
**Status:** 🔴 IN PROGRESS

---

## Current State: Documentation Chaos

### Files Found: 25 Markdown Files

| Location | Files | Purpose |
|----------|-------|---------|
| **Root** | README.md, QWEN.md, CLAUDE.md | Project overview, AI context |
| **docs/** | 10 files | Technical documentation |
| **gui/** | 1 file | GUI-specific docs |
| **reference/** | 13 files | Legacy Sonoma docs |

### Problems Identified

1. **Duplication:**
   - `CLAUDE.md` and `QWEN.md` have overlapping architecture info
   - `docs/register_map.md` duplicates info in `firmware/src/regs.rs`
   - `docs/HAL_API.md` duplicates actual code in `firmware/src/hal/`
   - `docs/FIRMWARE_API_FOR_GUI.md` duplicates `gui/src-tauri/src/fbc.rs`

2. **Outdated Info:**
   - `README.md` says "FBC System GUI: 0%" — but GUI is 100% complete
   - `docs/HAL_API.md` says "Status: Functional, needs optimization" — outdated
   - `docs/FIRMWARE_PL_AUDIT.md` and `docs/GAP_ANALYSIS.md` created today but overlap

3. **Scattered Info:**
   - Pattern converter info split across 4 files
   - Hardware status mentioned in 3+ places
   - Register map in docs/ but actual registers in firmware/src/

---

## Target Structure: Clean & Consolidated

### Tier 1: Entry Points (3 files)

| File | Purpose | Audience |
|------|---------|----------|
| `README.md` | Project overview, quick start | Everyone |
| `CLAUDE.md` | Ground truth, hardware status | AI assistants, developers |
| `docs/ARCHITECTURE.md` | System architecture, data flow | New team members |

### Tier 2: Technical Documentation (6 files)

| File | Purpose | Replaces |
|------|---------|----------|
| `docs/HARDWARE.md` | Hardware status, pinouts, power | GAP_ANALYSIS.md, FIRMWARE_PL_AUDIT.md sections |
| `docs/FIRMWARE.md` | Firmware architecture, HAL | HAL_API.md (move to code), FIRMWARE_API_FOR_GUI.md |
| `docs/FPGA.md` | RTL modules, bitstream build | GUI.md FPGA section |
| `docs/GUI.md` | GUI architecture, commands | GUI.md (consolidate) |
| `docs/PROTOCOL.md` | FBC protocol, commands | fbc_protocol.rs docs |
| `docs/REGISTER_MAP.md` | AXI registers (single source) | register_map.md (verify vs code) |

### Tier 3: Migration & Gaps (2 files)

| File | Purpose | Status |
|------|---------|--------|
| `docs/MIGRATION.md` | Legacy → FBC migration guide | PATTERN_CONVERTER_MIGRATION.md (rename) |
| `docs/GAPS.md` | Known gaps, implementation plans | GAP_ANALYSIS.md (rename) |

### Tier 4: Reference (External)

| Location | Purpose |
|----------|---------|
| `reference/sonoma_docs/` | Legacy Sonoma reference (READ ONLY) |
| `firmware/src/` | HAL API (code is the doc) |
| `rtl/` | RTL modules (code is the doc) |

---

## Consolidation Actions

### Action 1: Update README.md ✅ DONE
**Status:** ✅ Updated  
**Changes:**
- Fixed GUI status (0% → 100%)
- Added Pattern Converter Gap section
- Updated component progress table

### Action 2: Update CLAUDE.md ✅ DONE
**Status:** ✅ Updated  
**Changes:**
- Added hardware status table (PL programmed, PS not loaded)
- Added Pattern Converter Gap section
- Updated terminology (.hex vs .fbc)

### Action 3: Update QWEN.md ✅ DONE
**Status:** ✅ Updated  
**Changes:**
- Added hardware status table
- Added Pattern Converter Gap
- Updated bug list with `.fbc` gap

### Action 4: Create docs/ARCHITECTURE.md 🔴 TODO
**Purpose:** Single architecture overview  
**Content:**
- System block diagram
- Data flow (GUI → Ethernet → Firmware → AXI → FPGA → DUT)
- Component interaction
- Clock domains

**Source:** Merge CLAUDE.md architecture + QWEN.md architecture + README.md architecture

### Action 5: Create docs/HARDWARE.md 🔴 TODO
**Purpose:** Hardware status, what's programmed, what's not  
**Content:**
- PL status (programmed via JTAG)
- PS status (needs 12V + firmware load)
- Power requirements (12V @ TP16)
- JTAG pinout
- Test points

**Source:** GAP_ANALYSIS.md + FIRMWARE_PL_AUDIT.md + CLAUDE.md hardware status

### Action 6: Create docs/FIRMWARE.md 🔴 TODO
**Purpose:** Firmware architecture and usage  
**Content:**
- Boot sequence
- Main loop architecture
- HAL overview (link to code, don't duplicate)
- Protocol handlers
- Interrupt handling

**Source:** FIRMWARE_API_FOR_GUI.md + HAL_API.md (summarize, link to code)

### Action 7: Create docs/PROTOCOL.md 🔴 TODO
**Purpose:** FBC protocol specification  
**Content:**
- 28 commands with payloads
- Request/response format
- Error codes
- Timing diagrams

**Source:** firmware/src/fbc_protocol.rs (extract docs)

### Action 8: Verify docs/REGISTER_MAP.md 🔴 TODO
**Purpose:** Single source of truth for AXI registers  
**Action:**
- Compare vs `firmware/src/regs.rs`
- Update any mismatches
- Add note: "Code is authoritative"

### Action 9: Rename & Consolidate Migration Docs 🔴 TODO
**Action:**
- Rename `PATTERN_CONVERTER_MIGRATION.md` → `MIGRATION.md`
- Merge `GAP_ANALYSIS.md` → `GAPS.md`
- Delete `FIRMWARE_PL_AUDIT.md` (info moved to HARDWARE.md)

### Action 10: Delete Outdated Docs 🔴 TODO
**Files to Delete:**
- `docs/FIRMWARE_PL_AUDIT.md` (consolidated into HARDWARE.md)
- `docs/GAP_ANALYSIS.md` (renamed to GAPS.md)
- `docs/HAL_API.md` (outdated, code is authoritative)
- `docs/FIRMWARE_API_FOR_GUI.md` (outdated, merge into FIRMWARE.md)

---

## Documentation Principles

### 1. Code is Authoritative
- HAL API → Read `firmware/src/hal/*.rs`
- Protocol → Read `firmware/src/fbc_protocol.rs`
- Registers → Read `firmware/src/regs.rs`
- RTL → Read `rtl/*.v`

**Docs summarize, code defines.**

### 2. Single Source of Truth
- Architecture → `docs/ARCHITECTURE.md`
- Hardware status → `docs/HARDWARE.md`
- Protocol spec → `docs/PROTOCOL.md`
- Migration → `docs/MIGRATION.md`

**No duplication across files.**

### 3. Living Documents
- Update docs when code changes
- Add "Last verified" dates
- Link to code, don't copy

### 4. Clear Status Indicators
- ✅ Complete
- 🔴 In Progress
- ❌ Missing
- ⚠️ Known Issue

---

## Implementation Order

| Priority | Action | Effort | Impact |
|----------|--------|--------|--------|
| 🔴 HIGH | Create docs/ARCHITECTURE.md | 2 hours | High |
| 🔴 HIGH | Create docs/HARDWARE.md | 1 hour | High |
| 🟡 MED | Create docs/FIRMWARE.md | 3 hours | Medium |
| 🟡 MED | Create docs/PROTOCOL.md | 2 hours | Medium |
| 🟡 MED | Verify REGISTER_MAP.md | 1 hour | Medium |
| 🟢 LOW | Rename MIGRATION.md | 10 min | Low |
| 🟢 LOW | Delete outdated docs | 10 min | Low |

**Total Effort:** 9-10 hours  
**Priority:** Start with ARCHITECTURE.md and HARDWARE.md

---

## Next Steps

1. **Review this plan** — Confirm structure makes sense
2. **Implement in order** — Start with HIGH priority
3. **Verify against code** — Ensure docs match reality
4. **Delete old docs** — Clean up after consolidation

---

**Questions?**
- Which docs are most critical for your workflow?
- Any docs we should keep that I marked for deletion?
- Want me to start implementing now?
