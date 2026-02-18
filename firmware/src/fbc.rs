//! FBC (FORCE Bytecode) Definitions
//!
//! Matches the opcodes defined in FORCE CLI and FPGA RTL.

/// FBC opcodes
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FbcOpcode {
    /// No operation
    Nop = 0x00,
    /// End of program
    Halt = 0xFF,
    /// Loop next block N times
    LoopN = 0xB0,
    /// Repeat current pattern N times
    PatternRep = 0xB5,
    /// Generate sequence
    PatternSeq = 0xB6,
    /// Set pin values (128-bit payload)
    SetPins = 0xC0,
    /// Set output enables (128-bit payload)
    SetOen = 0xC1,
    /// Set both pins and OEN (256-bit payload)
    SetBoth = 0xC2,
    /// Wait N cycles
    Wait = 0xD0,
    /// Wait for external trigger
    Sync = 0xD1,
}

/// FBC instruction flags
pub mod flags {
    /// Last instruction in block
    pub const LAST: u8 = 0x01;
    /// Generate interrupt after execution
    pub const IRQ: u8 = 0x02;
    /// Part of loop body
    pub const LOOP: u8 = 0x04;
}

/// FBC instruction (64-bit)
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct FbcInstr {
    /// Operand (48 bits, stored in lower bytes)
    pub operand: u64,
}

impl FbcInstr {
    /// Create a new instruction
    pub const fn new(opcode: FbcOpcode, flags: u8, operand: u64) -> Self {
        let word = ((opcode as u64) << 56)
            | ((flags as u64) << 48)
            | (operand & 0x0000_FFFF_FFFF_FFFF);
        Self { operand: word }
    }

    /// Create NOP
    pub const fn nop() -> Self {
        Self::new(FbcOpcode::Nop, 0, 0)
    }

    /// Create HALT
    pub const fn halt() -> Self {
        Self::new(FbcOpcode::Halt, 0, 0)
    }

    /// Create PATTERN_REP
    pub const fn pattern_rep(count: u32) -> Self {
        Self::new(FbcOpcode::PatternRep, 0, count as u64)
    }

    /// Create WAIT
    pub const fn wait(cycles: u32) -> Self {
        Self::new(FbcOpcode::Wait, 0, cycles as u64)
    }

    /// Get opcode
    pub fn opcode(&self) -> u8 {
        ((self.operand >> 56) & 0xFF) as u8
    }

    /// Get flags
    pub fn flags(&self) -> u8 {
        ((self.operand >> 48) & 0xFF) as u8
    }

    /// Get operand
    pub fn operand_value(&self) -> u64 {
        self.operand & 0x0000_FFFF_FFFF_FFFF
    }

    /// Convert to raw u64
    pub fn to_u64(&self) -> u64 {
        self.operand
    }
}

/// FBC program (collection of instructions)
pub struct FbcProgram<'a> {
    pub instructions: &'a [FbcInstr],
}

impl<'a> FbcProgram<'a> {
    /// Create from instruction slice
    pub const fn new(instructions: &'a [FbcInstr]) -> Self {
        Self { instructions }
    }

    /// Get instruction count
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }

    /// Calculate total cycles (estimate based on PATTERN_REP)
    pub fn total_cycles(&self) -> u64 {
        let mut cycles = 0u64;
        for instr in self.instructions {
            match instr.opcode() {
                0xB5 => cycles += instr.operand_value(), // PATTERN_REP
                0xD0 => cycles += instr.operand_value(), // WAIT
                0xC0 | 0xC1 | 0xC2 => cycles += 1,       // SET_*
                _ => {}
            }
        }
        cycles
    }

    /// Calculate compression ratio
    pub fn compression_ratio(&self) -> f32 {
        if self.instructions.is_empty() {
            return 0.0;
        }
        self.total_cycles() as f32 / self.instructions.len() as f32
    }
}
