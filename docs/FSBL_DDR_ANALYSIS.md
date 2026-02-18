# FSBL DDR Analysis - ONETWO Decomposition

## Summary

Applied ONETWO methodology to reverse-engineer the FSBL.elf from Sonoma firmware.
Extracted the exact DDR initialization sequence for the FBC controller boards.

---

## The Boot Sequence (INVARIANT)

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Boot ROM      │────▶│     FSBL        │────▶│  Our Firmware   │
│  (in silicon)   │     │ (from SD card)  │     │  (from SD card) │
└─────────────────┘     └─────────────────┘     └─────────────────┘
       │                        │                        │
       ▼                        ▼                        ▼
   Loads FSBL              Inits DDR               Runs from DDR
   to OCM (256KB)          Loads app to DDR        at 0x00100000
```

**Key insight:** Our firmware runs from DDR (0x00100000). DDR must be initialized
first. The FSBL does this.

---

## Memory Map

| Region | Address | Size | Purpose |
|--------|---------|------|---------|
| DDR | 0x00000000 - 0x1FFFFFFF | 512MB | Main memory (our firmware lives here) |
| OCM | 0xFFFC0000 - 0xFFFFFFFF | 256KB | On-chip memory (FSBL runs here) |
| SLCR | 0xF8000000 - 0xF8000FFF | 4KB | System Level Control Registers |
| DDRC | 0xF8006000 - 0xF8006FFF | 4KB | DDR Controller |

---

## FSBL Functions (from disassembly)

| Address | Function | Purpose |
|---------|----------|---------|
| 0x0000 | _vector_table | ARM exception vectors |
| 0x00FC | _prestart | CPU init, cache invalidate |
| 0x0540 | NewDDROut32 | Write to DDR controller register |
| 0x058C | init_ddr | **DDR initialization sequence** |
| 0x07AC | FsblHookBeforeBitstreamDload | Pre-bitstream hook |
| 0x1928 | main | FSBL main loop |

---

## DDR Controller Register Writes (THE INVARIANT)

These are the exact values the FSBL writes to initialize DDR for this board.
These values are **board-specific** - derived from the DDR chip datasheet and
calibrated for the specific RAM on the FBC controller boards.

```
Address      Value        Register Name              Notes
────────────────────────────────────────────────────────────────────────
0xF8006000   0x00000200   ddrc_ctrl                  Initial (no enable)
0xF8006004   0x000C1061   Two_Rank_Cfg               Rank configuration
0xF800600C   0x03001001   HPR_Reg                    High priority read
0xF8006010   0x00014001   LPR_Reg                    Low priority read
0xF8006014   0x0004E020   WR_Reg                     Write configuration
0xF8006018   0x349B48CD   DRAM_Param_Reg0            *** TIMING ***
0xF800601C   0x820158A4   DRAM_Param_Reg1            *** TIMING ***
0xF8006020   0x250882C4   DRAM_Param_Reg2            *** TIMING ***
0xF8006028   0x00809004   DRAM_Param_Reg3            Refresh/ZQ settings
0xF800602C   0x00000000   DRAM_Param_Reg4            Reserved
0xF8006030   0x00040952   DRAM_Init_Param            Init sequence timing
0xF8006034   0x00020022   DRAM_EMR_Reg               Extended mode register
0xF8006040   0xFF000000   DRAM_EMR_MR_Reg            Mode register values
0xF8006044   0x0FF66666   DRAM_Burst8_Rdwr           Burst configuration
0xF8006050   0x00000256   DRAM_Disable_DQ            DQ disable timing
0xF800605C   0x00002223   DRAM_Addr_Map_Bank         Bank address mapping
0xF8006064   0x00020FE0   DRAM_Addr_Map_Col          Column address mapping
0xF80060A4   0x10200800   Phy_Cmd_Timeout_Rddata     PHY timeout
0xF80060B8   0x00200065   Phy_Ctrl_Sts_Reg           PHY control
0xF800617C   0x00000050   Phy_DLL_Lock_Diff_0        DLL lock threshold
0xF8006180   0x00000050   Phy_DLL_Lock_Diff_1        DLL lock threshold
0xF8006184   0x00000050   Phy_DLL_Lock_Diff_2        DLL lock threshold
0xF8006188   0x00000050   Phy_DLL_Lock_Diff_3        DLL lock threshold
0xF8006200   0x00000000   ECC_Scrub                  ECC disabled
0xF8006000   |= 0x01      ddrc_ctrl                  *** ENABLE DDR ***
```

---

## Timing Parameter Decode (DRAM_Param_Reg0 = 0x349B48CD)

Breaking down the timing register:

```
Bits [3:0]   = 0xD (13) = t_rfc_min (Refresh to activate)
Bits [7:4]   = 0xC (12) = t_rc (Row cycle time)
Bits [11:8]  = 0x8 (8)  = t_rrd (Row to row delay)
Bits [17:12] = 0x04 (4) = t_rp (Row precharge time)
Bits [21:18] = 0x1B     = t_ras_min
Bits [25:22] = 0x9      = t_ras_max
Bits [28:26] = 0x4      = t_faw
Bits [31:29] = 0x1      = reg_ddrc_pageclose_timer
```

These values correspond to DDR3-1066 or DDR3-1333 timing.

---

## What This Means for Your HAL

Your current HAL structure:

```
firmware/src/hal/
├── mod.rs          # Base register access
├── slcr.rs         # SLCR (0xF8000000) - clocks, MIO, resets
├── gpio.rs         # GPIO (0xE000A000)
├── i2c.rs          # I2C (0xE0004000/0xE0005000)
├── spi.rs          # SPI (0xE0006000/0xE0007000)
├── uart.rs         # UART (0xE0000000/0xE0001000)
├── xadc.rs         # XADC (0xF8007100)
├── pcap.rs         # PCAP/DEVCFG (0xF8007000)
└── ... (others)
```

**Missing:** DDR Controller (0xF8006000)

Your HAL is correct for **after DDR is initialized**. The SLCR controls
peripheral clocks and MIO - the DDR controller is a separate block.

---

## Options

### Option A: Keep Using Extracted FSBL (Recommended)

- FSBL.elf already works
- Contains calibrated DDR timing for your specific boards
- Your firmware runs after FSBL, DDR is ready
- Simple, proven, no risk

### Option B: Write Our Own FSBL in Rust

- Add `ddr.rs` to HAL with the exact values above
- Create minimal FSBL that runs from OCM
- Complete independence from Xilinx
- Risk: wrong timing = DDR doesn't work = board doesn't boot

### Option C: Hybrid - Verify DDR and Reinit if Needed

- Firmware checks if DDR is already initialized
- If yes (FSBL ran), skip init
- If no (JTAG debug), run init
- Best of both worlds

---

## Verification

To verify DDR is working:

```rust
// Read DDR controller status
let ddrc_ctrl = unsafe {
    core::ptr::read_volatile(0xF8006000 as *const u32)
};

// Bit 0 = DDR enabled
if ddrc_ctrl & 1 == 1 {
    // DDR is initialized (FSBL did its job)
} else {
    // Need to initialize DDR
}
```

---

## Files Referenced

- `reference/sonoma_extracted/v4.8C/FSBL.elf` - Working FSBL binary
- `reference/sonoma_extracted/v4.8C/BOOT.bin` - Working boot image
- `tools/bootgen/` - Our Rust bootgen replacement

---

## ONETWO Application

**ONE (Decompose):**
- Disassembled FSBL.elf
- Found init_ddr function
- Extracted register write sequence
- Identified the INVARIANT (timing values)

**TWO (Compose):**
- Can now write equivalent Rust code
- Can create our own FSBL
- Can verify DDR init in firmware

The FSBL is just a **register write sequence** - nothing magical.
The magic numbers are board-specific DDR timing calibration.
