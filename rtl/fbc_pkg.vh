`ifndef FBC_PKG_VH
`define FBC_PKG_VH

//=============================================================================
// FBC Semiconductor System - Package Definitions
//=============================================================================

// Version
`define FBC_VERSION 32'h0001_0000  // v1.0.0

// Vector dimensions
`define VECTOR_WIDTH    128   // BIM-compatible DUT I/O pins
`define FAST_WIDTH      32    // Fast vector pins (direct, no BIM)
`define PIN_COUNT       160   // Total pins (128 + 32)
`define REPEAT_WIDTH    32    // Max repeat count

// Pin banks (matches hardware)
`define BANK13_START    0     // gpio[0:47]   - 48 pins (BIM)
`define BANK13_END      47
`define BANK33_START    48    // gpio[48:95]  - 48 pins (BIM)
`define BANK33_END      95
`define BANK34_START    96    // gpio[96:127] - 32 pins (BIM)
`define BANK34_END      127

// Bank 35 - Direct FPGA pins (no BIM, directly routed)
// All 32 pins available for fast/trigger/debug use
`define BANK35_START    128   // gpio[128:159] - 32 direct pins
`define BANK35_END      159
`define BANK35_COUNT    32

// Special purpose pin assignments (optional use)
`define FAST_SCOPE_TRIG   128   // Scope trigger output
`define FAST_ERROR_STROBE 129   // Error strobe output
`define FAST_SYNC_N       130   // LVDS sync N
`define FAST_SYNC_P       131   // LVDS sync P
`define FAST_SYSCLK_N     136   // SYSCLK0_N (clock input capable)
`define FAST_SYSCLK_P     137   // SYSCLK0_P (clock input capable)

//=============================================================================
// FBC Opcodes (matches FORCE CLI fbc.hpp)
//=============================================================================

// Control flow
`define FBC_NOP         8'h00   // No operation
`define FBC_HALT        8'hFF   // End of program

// Pattern operations
`define FBC_LOOP_N      8'hB0   // Loop next block N times
`define FBC_PATTERN_REP 8'hB5   // Repeat current pattern: operand = total_repeats - 1
                                 //   (vector_engine.v subtracts 1 more because first output is immediate)
                                 //   Example: repeat 5× → encoder sets operand=4 → RTL loads 3 → 4 counted + 1 immediate = 5
`define FBC_PATTERN_SEQ 8'hB6   // Generate sequence

// Pin control
`define FBC_SET_PINS    8'hC0   // Set pin values (128-bit payload follows)
`define FBC_SET_OEN     8'hC1   // Set output enables (128-bit payload follows)
`define FBC_SET_BOTH    8'hC2   // Set both pins and OEN (256-bit payload)

// Timing
`define FBC_WAIT        8'hD0   // Wait N cycles
`define FBC_SYNC        8'hD1   // Wait for external trigger

// Immediate data
`define FBC_IMM32       8'hE0   // 32-bit immediate follows
`define FBC_IMM128      8'hE1   // 128-bit immediate follows

//=============================================================================
// FBC Instruction Format
//=============================================================================
//
// Basic instruction: 64 bits
// ┌────────┬────────┬────────────────────────────────────────────┐
// │ opcode │ flags  │              operand                       │
// │ [63:56]│ [55:48]│              [47:0]                        │
// └────────┴────────┴────────────────────────────────────────────┘
//
// Extended instruction: 64 + 128 bits (for SET_PINS, SET_OEN)
// ┌────────┬────────┬──────────┐┌──────────────────────────────┐
// │ opcode │ flags  │ reserved ││     128-bit payload          │
// └────────┴────────┴──────────┘└──────────────────────────────┘
//
//=============================================================================

// Flags
`define FBC_FLAG_LAST   8'h01   // Last instruction in block
`define FBC_FLAG_IRQ    8'h02   // Generate interrupt after
`define FBC_FLAG_LOOP   8'h04   // Part of loop body

//=============================================================================
// Pin Types (compatible with legacy)
//=============================================================================

`define PIN_TYPE_BIDI       4'h0  // Bidirectional
`define PIN_TYPE_INPUT      4'h1  // Input only
`define PIN_TYPE_OUTPUT     4'h2  // Output only
`define PIN_TYPE_OPEN_C     4'h3  // Open collector
`define PIN_TYPE_PULSE      4'h4  // Pulse output
`define PIN_TYPE_NPULSE     4'h5  // Inverted pulse
`define PIN_TYPE_ERR_TRIG   4'h6  // Error trigger
`define PIN_TYPE_VEC_CLK    4'h7  // Vector clock out
`define PIN_TYPE_VEC_CLK_EN 4'h8  // Vector clock enable

//=============================================================================
// AXI Configuration
//=============================================================================

`define AXI_DATA_WIDTH  32
`define AXI_ADDR_WIDTH  14    // 16KB address space per peripheral

// Base addresses (directly memory mapped)
`define AXI_FBC_CTRL_BASE   32'h4004_0000   // FBC control registers
`define AXI_PIN_CTRL_BASE   32'h4005_0000   // Pin configuration
`define AXI_STATUS_BASE     32'h4006_0000   // Status & error registers
`define AXI_FREQ_BASE       32'h4007_0000   // Frequency counters
`define AXI_CLK_CTRL_BASE   32'h4008_0000   // Clock control (ONETWO freq select)
`define AXI_DNA_BASE        32'h400A_0000   // Device DNA (57-bit silicon ID, read-only)

//=============================================================================
// Error handling
//=============================================================================

`define MAX_ERROR_COUNT     1024    // Max errors before auto-stop
`define ERROR_BRAM_DEPTH    1024    // Error buffer size

//=============================================================================
// Timing
//=============================================================================

`define HOLD_TIME 1   // Simulation hold time (ns)

`endif // FBC_PKG_VH
