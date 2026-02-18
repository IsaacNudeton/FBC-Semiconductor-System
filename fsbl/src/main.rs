//! FBC First Stage Boot Loader (FSBL) - Pure Rust
//!
//! Runs from OCM, initializes DDR, programs FPGA, loads application, jumps to it.
//!
//! Boot sequence:
//! 1. Boot ROM loads us to 0x0 (OCM mapped here at boot)
//! 2. We initialize DDR controller with board-specific timing
//! 3. We program FPGA with bitstream (via PCAP/DEVCFG)
//! 4. We read SD card, find application partition
//! 5. We copy application to DDR (0x00100000)
//! 6. We jump to application entry point
//!
//! Size target: < 64KB (plenty of room in 192KB OCM)
//!
//! BOOT.FBC format:
//!   [Boot Header] [FSBL] [Bitstream] [Firmware]

#![no_std]
#![no_main]

use core::ptr::{read_volatile, write_volatile};

// =============================================================================
// Memory-Mapped Registers
// =============================================================================

const SLCR_BASE: usize = 0xF800_0000;
const SLCR_UNLOCK: usize = SLCR_BASE + 0x008;
const SLCR_LOCK: usize = SLCR_BASE + 0x004;
const UNLOCK_KEY: u32 = 0xDF0D;
const LOCK_KEY: u32 = 0x767B;

const DDRC_BASE: usize = 0xF800_6000;
const UART0_BASE: usize = 0xE000_0000;

const SD_BASE: usize = 0xE010_0000;  // SDIO0
const DEVCFG_BASE: usize = 0xF800_7000;  // Device Configuration (PCAP)

// SLCR registers for FPGA
const SLCR_FPGA_RST_CTRL: usize = SLCR_BASE + 0x240;
const SLCR_LVL_SHFTR_EN: usize = SLCR_BASE + 0x900;

// DEVCFG registers
const DEVCFG_CTRL: usize = DEVCFG_BASE + 0x00;
const DEVCFG_INT_STS: usize = DEVCFG_BASE + 0x0C;
const DEVCFG_STATUS: usize = DEVCFG_BASE + 0x14;
const DEVCFG_DMA_SRC: usize = DEVCFG_BASE + 0x18;
const DEVCFG_DMA_DST: usize = DEVCFG_BASE + 0x1C;
const DEVCFG_DMA_SRC_LEN: usize = DEVCFG_BASE + 0x20;
const DEVCFG_DMA_DST_LEN: usize = DEVCFG_BASE + 0x24;
const DEVCFG_MCTRL: usize = DEVCFG_BASE + 0x80;

// =============================================================================
// DDR Timing Values (extracted from Sonoma FSBL via ONETWO)
// =============================================================================

const DDR_CONFIG: [(usize, u32); 25] = [
    (0x000, 0x0000_0200),  // ddrc_ctrl (disabled)
    (0x004, 0x000C_1061),  // two_rank_cfg
    (0x00C, 0x0300_1001),  // hpr_reg
    (0x010, 0x0001_4001),  // lpr_reg
    (0x014, 0x0004_E020),  // wr_reg
    (0x018, 0x349B_48CD),  // dram_param0 - TIMING
    (0x01C, 0x8201_58A4),  // dram_param1 - TIMING
    (0x020, 0x2508_82C4),  // dram_param2 - TIMING
    (0x028, 0x0080_9004),  // dram_param3
    (0x02C, 0x0000_0000),  // dram_param4
    (0x030, 0x0004_0952),  // dram_init_param
    (0x034, 0x0002_0022),  // dram_emr
    (0x040, 0xFF00_0000),  // dram_emr_mr
    (0x044, 0x0FF6_6666),  // dram_burst8
    (0x050, 0x0000_0256),  // dram_disable_dq
    (0x05C, 0x0000_2223),  // dram_addr_map_bank
    (0x064, 0x0002_0FE0),  // dram_addr_map_col
    (0x0A4, 0x1020_0800),  // phy_cmd_timeout
    (0x0B8, 0x0020_0065),  // phy_ctrl_sts
    (0x17C, 0x0000_0050),  // phy_dll_lock0
    (0x180, 0x0000_0050),  // phy_dll_lock1
    (0x184, 0x0000_0050),  // phy_dll_lock2
    (0x188, 0x0000_0050),  // phy_dll_lock3
    (0x200, 0x0000_0000),  // ecc_scrub
    (0x000, 0x0000_0201),  // ddrc_ctrl - ENABLE
];

// =============================================================================
// Register Access
// =============================================================================

#[inline(always)]
unsafe fn write_reg(addr: usize, val: u32) {
    write_volatile(addr as *mut u32, val);
}

#[inline(always)]
unsafe fn read_reg(addr: usize) -> u32 {
    read_volatile(addr as *const u32)
}

// =============================================================================
// SLCR (System Level Control)
// =============================================================================

unsafe fn slcr_unlock() {
    write_reg(SLCR_UNLOCK, UNLOCK_KEY);
}

unsafe fn slcr_lock() {
    write_reg(SLCR_LOCK, LOCK_KEY);
}

// =============================================================================
// DDR Initialization
// =============================================================================

unsafe fn init_ddr() {
    // Check if DDR already initialized (e.g., JTAG debug)
    let ctrl = read_reg(DDRC_BASE);
    if ctrl & 1 == 1 {
        return; // Already running
    }

    // Write all DDR configuration registers
    for &(offset, value) in &DDR_CONFIG {
        write_reg(DDRC_BASE + offset, value);
    }

    // Wait for DDR to stabilize
    delay_us(1000);

    // Verify DDR is enabled
    let ctrl = read_reg(DDRC_BASE);
    if ctrl & 1 != 1 {
        // DDR init failed - hang with pattern
        hang();
    }
}

// =============================================================================
// FPGA Programming (via PCAP/DEVCFG)
// =============================================================================

/// Program FPGA with bitstream from memory
/// bitstream_addr: address in DDR where bitstream is loaded
/// bitstream_len: length in bytes
unsafe fn program_fpga(bitstream_addr: u32, bitstream_len: u32) -> bool {
    // 1. Enable level shifters (PS <-> PL interface)
    write_reg(SLCR_LVL_SHFTR_EN, 0xF);

    // 2. Assert FPGA resets
    write_reg(SLCR_FPGA_RST_CTRL, 0xF);
    delay_us(100);

    // 3. Enable PCAP clock and configure DEVCFG
    let ctrl = read_reg(DEVCFG_CTRL);
    // Set PCAP_MODE, PCAP_PR, clear QUARTER_PCAP_RATE
    let ctrl = (ctrl | (1 << 26) | (1 << 27)) & !(1 << 25);
    write_reg(DEVCFG_CTRL, ctrl);

    // 4. Clear interrupts
    write_reg(DEVCFG_INT_STS, 0xFFFF_FFFF);

    // 5. Deassert PROG_B (allow programming)
    let mctrl = read_reg(DEVCFG_MCTRL);
    write_reg(DEVCFG_MCTRL, mctrl | (1 << 4)); // PCAP_PR

    // 6. Wait for PCAP to be ready
    for _ in 0..100000 {
        let status = read_reg(DEVCFG_STATUS);
        if status & (1 << 4) != 0 { // PCAP_INIT
            break;
        }
    }

    // 7. Setup DMA transfer
    // Source: bitstream in DDR
    // Destination: 0xFFFF_FFFF (PCAP)
    let word_len = (bitstream_len + 3) / 4;
    write_reg(DEVCFG_DMA_SRC, bitstream_addr);
    write_reg(DEVCFG_DMA_DST, 0xFFFF_FFFF);
    write_reg(DEVCFG_DMA_SRC_LEN, word_len);
    write_reg(DEVCFG_DMA_DST_LEN, word_len);

    // 8. Wait for DMA complete
    for _ in 0..10_000_000 {
        let int_sts = read_reg(DEVCFG_INT_STS);
        if int_sts & (1 << 2) != 0 { // DMA_DONE
            break;
        }
        if int_sts & 0x3C0 != 0 { // Any error
            return false;
        }
    }

    // 9. Wait for FPGA DONE
    for _ in 0..100000 {
        let int_sts = read_reg(DEVCFG_INT_STS);
        if int_sts & (1 << 0) != 0 { // FPGA_DONE
            break;
        }
    }

    // 10. Deassert FPGA resets
    write_reg(SLCR_FPGA_RST_CTRL, 0x0);
    delay_us(100);

    // 11. Check if programming succeeded
    let int_sts = read_reg(DEVCFG_INT_STS);
    (int_sts & (1 << 0)) != 0 // FPGA_DONE set
}

// =============================================================================
// Simple UART (for debug output)
// =============================================================================

const UART_SR: usize = 0x2C;   // Status register
const UART_FIFO: usize = 0x30; // TX/RX FIFO
const UART_SR_TXFULL: u32 = 1 << 4;

unsafe fn uart_putc(c: u8) {
    // Wait for TX FIFO not full
    while read_reg(UART0_BASE + UART_SR) & UART_SR_TXFULL != 0 {}
    write_reg(UART0_BASE + UART_FIFO, c as u32);
}

unsafe fn uart_puts(s: &[u8]) {
    for &c in s {
        if c == b'\n' {
            uart_putc(b'\r');
        }
        uart_putc(c);
    }
}

// =============================================================================
// SD Card (minimal implementation)
// =============================================================================
//
// NOTE: This is raw sector access, not FAT filesystem access.
// For development/testing, write boot image to raw SD:
//   dd if=BOOT.FBC of=/dev/sdX bs=512
//
// For production with FAT, need to either:
// 1. Implement minimal FAT16/32 (adds ~2KB code)
// 2. Store boot image at fixed sector offset after FAT
// 3. Use Boot ROM's ability to load additional partitions
//
// TODO: Implement FAT support for user-friendly SD card use.

const SD_CMD: usize = 0x0C;
const SD_ARG: usize = 0x08;
const SD_RSP0: usize = 0x10;
const SD_PSTATE: usize = 0x24;
const SD_BLK_SIZE: usize = 0x04;
const SD_BLK_CNT: usize = 0x06;
const SD_XFER_MODE: usize = 0x0C;
const SD_BUFFER: usize = 0x20;
const SD_INT_STS: usize = 0x30;

unsafe fn sd_wait_ready() -> bool {
    for _ in 0..100000 {
        let pstate = read_reg(SD_BASE + SD_PSTATE);
        if pstate & 0x3 == 0 { // CMD and DAT lines free
            return true;
        }
    }
    false
}

unsafe fn sd_send_cmd(cmd: u32, arg: u32) -> u32 {
    // Clear interrupts
    write_reg(SD_BASE + SD_INT_STS, 0xFFFF_FFFF);

    // Set argument
    write_reg(SD_BASE + SD_ARG, arg);

    // Send command
    write_reg(SD_BASE + SD_CMD, cmd);

    // Wait for completion
    for _ in 0..100000 {
        let sts = read_reg(SD_BASE + SD_INT_STS);
        if sts & 0x1 != 0 { // Command complete
            return read_reg(SD_BASE + SD_RSP0);
        }
        if sts & 0x8000 != 0 { // Error
            return 0xFFFF_FFFF;
        }
    }
    0xFFFF_FFFF
}

unsafe fn sd_read_block(sector: u32, buf: &mut [u8; 512]) -> bool {
    if !sd_wait_ready() {
        return false;
    }

    // Set block size and count
    write_reg(SD_BASE + SD_BLK_SIZE, 512);
    write_reg(SD_BASE + SD_BLK_CNT, 1);

    // Read single block (CMD17)
    let cmd = (17 << 8) | 0x3A; // CMD17, response R1, data read
    write_reg(SD_BASE + SD_XFER_MODE, 0x10); // Single block read
    let _rsp = sd_send_cmd(cmd, sector);

    // Wait for data
    for _ in 0..100000 {
        let sts = read_reg(SD_BASE + SD_INT_STS);
        if sts & 0x20 != 0 { // Buffer read ready
            // Read 512 bytes from buffer
            for i in 0..128 {
                let word = read_reg(SD_BASE + SD_BUFFER);
                buf[i*4] = (word & 0xFF) as u8;
                buf[i*4 + 1] = ((word >> 8) & 0xFF) as u8;
                buf[i*4 + 2] = ((word >> 16) & 0xFF) as u8;
                buf[i*4 + 3] = ((word >> 24) & 0xFF) as u8;
            }
            return true;
        }
        if sts & 0x8000 != 0 {
            return false;
        }
    }
    false
}

// =============================================================================
// BOOT.BIN Parsing
// =============================================================================

/// Partition info: (data_offset, data_size, load_addr, exec_addr)
struct PartitionInfo {
    offset: u32,
    size: u32,
    load: u32,
    exec: u32,
}

/// Find partitions in boot image
/// Returns (bitstream_partition, app_partition)
unsafe fn find_partitions(boot_data: &[u8]) -> (Option<PartitionInfo>, Option<PartitionInfo>) {
    // Check for valid boot header
    let magic = u32::from_le_bytes([boot_data[0x24], boot_data[0x25],
                                     boot_data[0x26], boot_data[0x27]]);
    if magic != 0x584C4E58 { // "XLNX"
        return (None, None);
    }

    // Partition headers start at 0xC80
    // Each partition header is 0x40 bytes
    // Partition 0 = FSBL (skip)
    // Partition 1 = Bitstream (if present) or App
    // Partition 2 = App (if bitstream present)

    let mut bitstream: Option<PartitionInfo> = None;
    let mut app: Option<PartitionInfo> = None;

    // Read partition count from IHT
    let iht_offset = u32::from_le_bytes([boot_data[0x98], boot_data[0x99],
                                          boot_data[0x9A], boot_data[0x9B]]) as usize;

    let part_count = if iht_offset + 4 < boot_data.len() {
        u32::from_le_bytes([boot_data[iht_offset + 4], boot_data[iht_offset + 5],
                            boot_data[iht_offset + 6], boot_data[iht_offset + 7]])
    } else {
        2 // Assume FSBL + App minimum
    };

    // Parse partitions starting from index 1 (skip FSBL at 0)
    for i in 1..part_count.min(4) as usize {
        let ph_off = 0xC80 + i * 0x40;
        if ph_off + 0x40 > boot_data.len() {
            break;
        }

        let data_off = u32::from_le_bytes([boot_data[ph_off + 0x14], boot_data[ph_off + 0x15],
                                            boot_data[ph_off + 0x16], boot_data[ph_off + 0x17]]) * 4;
        let data_len = u32::from_le_bytes([boot_data[ph_off + 0x04], boot_data[ph_off + 0x05],
                                            boot_data[ph_off + 0x06], boot_data[ph_off + 0x07]]) * 4;
        let load_addr = u32::from_le_bytes([boot_data[ph_off + 0x10], boot_data[ph_off + 0x11],
                                             boot_data[ph_off + 0x12], boot_data[ph_off + 0x13]]);
        let exec_addr = u32::from_le_bytes([boot_data[ph_off + 0x18], boot_data[ph_off + 0x19],
                                             boot_data[ph_off + 0x1A], boot_data[ph_off + 0x1B]]);

        let info = PartitionInfo {
            offset: data_off,
            size: data_len,
            load: load_addr,
            exec: exec_addr,
        };

        // Detect bitstream by checking first word of data (sync word 0xAA995566)
        if data_off as usize + 4 <= boot_data.len() {
            let first_word = u32::from_le_bytes([
                boot_data[data_off as usize],
                boot_data[data_off as usize + 1],
                boot_data[data_off as usize + 2],
                boot_data[data_off as usize + 3],
            ]);

            if first_word == 0xAA995566 || first_word == 0x665599AA {
                bitstream = Some(info);
            } else if load_addr >= 0x00100000 {
                // Looks like application (loads to DDR)
                app = Some(info);
            }
        }
    }

    (bitstream, app)
}

// =============================================================================
// Boot Image Loading
// =============================================================================

/// Load entire boot image to DDR for parsing
/// Returns the number of bytes loaded
unsafe fn load_boot_image(ddr_addr: usize, max_size: usize) -> Option<usize> {
    let mut offset = 0usize;

    while offset < max_size {
        let mut buf = [0u8; 512];
        let sector = (offset / 512) as u32;

        if !sd_read_block(sector, &mut buf) {
            if offset == 0 {
                return None; // Can't read first sector
            }
            break; // End of file
        }

        // Copy to DDR
        for j in 0..512 {
            write_volatile((ddr_addr + offset + j) as *mut u8, buf[j]);
        }

        offset += 512;

        // On first read, check file size from partition headers
        if offset == 512 {
            // Quick check - just load enough for headers + partitions
            // We'll get actual sizes from partition headers later
        }
    }

    Some(offset)
}

/// Load partition data from boot image (already in DDR) to its load address
unsafe fn load_partition(boot_base: usize, part: &PartitionInfo) -> bool {
    let src = boot_base + part.offset as usize;
    let dst = part.load as usize;
    let len = part.size as usize;

    // Copy from boot image in DDR to load address
    for i in 0..len {
        let byte = read_volatile((src + i) as *const u8);
        write_volatile((dst + i) as *mut u8, byte);
    }

    true
}

// =============================================================================
// Utilities
// =============================================================================

fn delay_us(us: u32) {
    // Approximate delay (CPU at ~667MHz initially, ~3 cycles per loop)
    let cycles = us * 200;
    for _ in 0..cycles {
        core::hint::spin_loop();
    }
}

fn hang() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

// =============================================================================
// Entry Point
// =============================================================================

// Boot image staging area in DDR (high address to avoid conflicts)
const BOOT_IMAGE_ADDR: usize = 0x0800_0000; // 128MB mark
const BOOT_IMAGE_MAX: usize = 0x0100_0000;  // 16MB max boot image

#[no_mangle]
pub extern "C" fn main() -> ! {
    unsafe {
        // 1. Unlock SLCR
        slcr_unlock();

        // 2. Initialize UART for debug (assuming already configured by Boot ROM)
        uart_puts(b"\nFBC FSBL v1.1\n");

        // 3. Initialize DDR
        uart_puts(b"Init DDR...\n");
        init_ddr();
        uart_puts(b"DDR OK\n");

        // 4. Quick DDR test (write/read)
        let test_addr = 0x0010_0000 as *mut u32;
        write_volatile(test_addr, 0xDEAD_BEEF);
        let readback = read_volatile(test_addr);
        if readback != 0xDEAD_BEEF {
            uart_puts(b"DDR TEST FAIL\n");
            hang();
        }
        uart_puts(b"DDR test OK\n");

        // 5. Load boot image from SD to DDR staging area
        uart_puts(b"Loading BOOT.FBC...\n");
        let boot_size = match load_boot_image(BOOT_IMAGE_ADDR, BOOT_IMAGE_MAX) {
            Some(size) => size,
            None => {
                uart_puts(b"SD read failed\n");
                hang();
            }
        };
        uart_puts(b"Loaded ");
        uart_print_hex(boot_size as u32);
        uart_puts(b" bytes\n");

        // 6. Parse partitions from boot image
        let boot_slice = core::slice::from_raw_parts(
            BOOT_IMAGE_ADDR as *const u8,
            boot_size
        );
        let (bitstream, app) = find_partitions(boot_slice);

        // 7. Program FPGA if bitstream present
        if let Some(ref bs) = bitstream {
            uart_puts(b"Programming FPGA...\n");

            // Bitstream is already in DDR at boot_image + offset
            let bs_addr = (BOOT_IMAGE_ADDR + bs.offset as usize) as u32;

            if !program_fpga(bs_addr, bs.size) {
                uart_puts(b"FPGA program failed!\n");
                hang();
            }
            uart_puts(b"FPGA OK\n");
        } else {
            uart_puts(b"No bitstream\n");
        }

        // 8. Load application to its load address
        let entry = match app {
            Some(ref app_part) => {
                uart_puts(b"Loading app to ");
                uart_print_hex(app_part.load);
                uart_puts(b"...\n");

                if !load_partition(BOOT_IMAGE_ADDR, app_part) {
                    uart_puts(b"App load failed\n");
                    hang();
                }
                app_part.exec
            }
            None => {
                uart_puts(b"No app partition!\n");
                hang();
            }
        };

        // 9. Lock SLCR
        slcr_lock();

        // 10. Jump to application
        uart_puts(b"Jump to ");
        uart_print_hex(entry);
        uart_puts(b"\n\n");

        let app_entry: extern "C" fn() -> ! = core::mem::transmute(entry as usize);
        app_entry();
    }
}

/// Print a hex value to UART (for debug)
unsafe fn uart_print_hex(val: u32) {
    const HEX: &[u8] = b"0123456789ABCDEF";
    uart_puts(b"0x");
    for i in (0..8).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        uart_putc(HEX[nibble]);
    }
}

// =============================================================================
// Startup Code
// =============================================================================

core::arch::global_asm!(r#"
.section .vectors, "ax"
.global _vectors
_vectors:
    b   _start          @ Reset
    b   .               @ Undefined
    b   .               @ SVC
    b   .               @ Prefetch abort
    b   .               @ Data abort
    nop                 @ Reserved
    b   .               @ IRQ
    b   .               @ FIQ

.section .text.boot, "ax"
.global _start
_start:
    @ Disable interrupts
    cpsid   if

    @ Set stack pointer
    ldr     sp, =_stack_top

    @ Clear BSS
    ldr     r0, =_bss_start
    ldr     r1, =_bss_end
    mov     r2, #0
1:
    cmp     r0, r1
    bge     2f
    str     r2, [r0], #4
    b       1b
2:
    @ Jump to Rust main
    bl      main

    @ Should never return
3:
    wfi
    b       3b
"#);

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    hang()
}
