//! FBC Simple Text Vector Format (.fvec)
//!
//! Human-readable format for testing and manual vector creation.
//!
//! # Format
//!
//! ```text
//! # Comment lines start with #
//!
//! # Header section
//! CLOCK 100000000   # Vector clock in Hz
//!
//! # Pin configuration (optional)
//! PIN 0 INPUT       # Pin 0 is input (compare)
//! PIN 1 OUTPUT      # Pin 1 is output (drive)
//! PIN 2 BIDI        # Pin 2 is bidirectional (default)
//! PIN 3 PULSE       # Pin 3 is pulse
//!
//! # Vector section - one of:
//!
//! # Binary (160 chars of 0/1/X/Z):
//! 01010101...  REPEAT 1000   # Optional repeat count
//!
//! # Hex (40 chars, prefix 0x):
//! 0x0102030405060708090a0b0c0d0e0f1011121314
//!
//! # Sparse (list changed pins from previous):
//! TOGGLE 0 5 10 15   # Toggle pins 0, 5, 10, 15
//!
//! # Special:
//! ZERO               # All pins low
//! ONES               # All pins high
//! ```
//!
//! # Example File
//!
//! ```text
//! # Test pattern for blink test
//! CLOCK 10000000
//!
//! PIN 0-7 OUTPUT
//! PIN 8-15 INPUT
//!
//! # Walking ones pattern
//! 1000000000000000...  REPEAT 1
//! 0100000000000000...  REPEAT 1
//! 0010000000000000...  REPEAT 1
//! # ... etc
//! ```

use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::fs::File;

use super::format::{Vector, PinType, PinConfig, PIN_COUNT};

/// Parsed FVEC file
#[derive(Debug, Clone)]
pub struct FvecProgram {
    pub clock_hz: u32,
    pub pin_config: PinConfig,
    pub vectors: Vec<FvecVector>,
}

/// A vector entry from FVEC file
#[derive(Debug, Clone)]
pub struct FvecVector {
    pub vector: Vector,
    pub repeat: u32,
    pub label: Option<String>,
}

impl Default for FvecProgram {
    fn default() -> Self {
        Self {
            clock_hz: 100_000_000,
            pin_config: PinConfig::default(),
            vectors: Vec::new(),
        }
    }
}

impl FvecProgram {
    /// Parse FVEC from file
    pub fn from_file(path: &Path) -> Result<Self, FvecError> {
        let file = File::open(path).map_err(FvecError::Io)?;
        Self::from_reader(BufReader::new(file))
    }

    /// Parse FVEC from string
    pub fn from_str(s: &str) -> Result<Self, FvecError> {
        Self::from_reader(s.as_bytes())
    }

    /// Parse FVEC from any reader
    pub fn from_reader<R: Read>(reader: R) -> Result<Self, FvecError> {
        let mut program = FvecProgram::default();
        let mut line_num = 0;
        let mut prev_vector = Vector::ZERO;
        let mut current_label: Option<String> = None;

        for line_result in BufReader::new(reader).lines() {
            line_num += 1;
            let line = line_result.map_err(FvecError::Io)?;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            // Parse the line
            match parse_line(line, &prev_vector, &mut current_label) {
                Ok(ParsedLine::Clock(hz)) => {
                    program.clock_hz = hz;
                }
                Ok(ParsedLine::Pin(pin, pin_type)) => {
                    program.pin_config.types[pin] = pin_type;
                }
                Ok(ParsedLine::PinRange(start, end, pin_type)) => {
                    for pin in start..=end {
                        if pin < PIN_COUNT {
                            program.pin_config.types[pin] = pin_type;
                        }
                    }
                }
                Ok(ParsedLine::Vector(vec, repeat)) => {
                    program.vectors.push(FvecVector {
                        vector: vec,
                        repeat,
                        label: current_label.take(),
                    });
                    prev_vector = vec;
                }
                Ok(ParsedLine::Label(name)) => {
                    current_label = Some(name);
                }
                Ok(ParsedLine::Skip) => {}
                Err(msg) => {
                    return Err(FvecError::Parse { line: line_num, msg });
                }
            }
        }

        Ok(program)
    }

    /// Total vector count (after expanding repeats)
    pub fn total_vectors(&self) -> u64 {
        self.vectors.iter().map(|v| v.repeat as u64).sum()
    }
}

/// Parsed line result
enum ParsedLine {
    Clock(u32),
    Pin(usize, PinType),
    PinRange(usize, usize, PinType),
    Vector(Vector, u32),
    Label(String),
    Skip,
}

/// Parse a single line
fn parse_line(
    line: &str,
    prev: &Vector,
    current_label: &mut Option<String>,
) -> Result<ParsedLine, String> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(ParsedLine::Skip);
    }

    match parts[0].to_uppercase().as_str() {
        "CLOCK" => {
            if parts.len() < 2 {
                return Err("CLOCK requires frequency value".into());
            }
            let hz = parts[1]
                .replace("_", "")
                .parse::<u32>()
                .map_err(|e| format!("Invalid clock frequency: {}", e))?;
            Ok(ParsedLine::Clock(hz))
        }

        "PIN" => {
            if parts.len() < 3 {
                return Err("PIN requires pin number/range and type".into());
            }

            let pin_spec = parts[1];
            let pin_type = parse_pin_type(parts[2])?;

            // Check for range (e.g., "0-7")
            if pin_spec.contains('-') {
                let range: Vec<&str> = pin_spec.split('-').collect();
                if range.len() != 2 {
                    return Err(format!("Invalid pin range: {}", pin_spec));
                }
                let start = range[0].parse::<usize>()
                    .map_err(|e| format!("Invalid pin number: {}", e))?;
                let end = range[1].parse::<usize>()
                    .map_err(|e| format!("Invalid pin number: {}", e))?;
                if start >= PIN_COUNT || end >= PIN_COUNT {
                    return Err(format!("Pin number out of range (0-{})", PIN_COUNT - 1));
                }
                Ok(ParsedLine::PinRange(start, end, pin_type))
            } else {
                let pin = pin_spec.parse::<usize>()
                    .map_err(|e| format!("Invalid pin number: {}", e))?;
                if pin >= PIN_COUNT {
                    return Err(format!("Pin number out of range (0-{})", PIN_COUNT - 1));
                }
                Ok(ParsedLine::Pin(pin, pin_type))
            }
        }

        "ZERO" => {
            let repeat = parse_repeat(&parts)?;
            Ok(ParsedLine::Vector(Vector::ZERO, repeat))
        }

        "ONES" => {
            let repeat = parse_repeat(&parts)?;
            Ok(ParsedLine::Vector(Vector::ONES, repeat))
        }

        "TOGGLE" => {
            // Toggle specified pins from previous vector
            if parts.len() < 2 {
                return Err("TOGGLE requires at least one pin number".into());
            }

            let mut vec = *prev;
            for pin_str in &parts[1..] {
                // Stop at REPEAT keyword
                if pin_str.to_uppercase() == "REPEAT" {
                    break;
                }
                let pin = pin_str.parse::<usize>()
                    .map_err(|e| format!("Invalid pin number '{}': {}", pin_str, e))?;
                if pin >= PIN_COUNT {
                    return Err(format!("Pin {} out of range (0-{})", pin, PIN_COUNT - 1));
                }
                vec.set_bit(pin, !vec.get_bit(pin));
            }

            let repeat = parse_repeat(&parts)?;
            Ok(ParsedLine::Vector(vec, repeat))
        }

        "LABEL" => {
            if parts.len() < 2 {
                return Err("LABEL requires a name".into());
            }
            Ok(ParsedLine::Label(parts[1].to_string()))
        }

        s if s.starts_with("0X") || s.starts_with("0x") => {
            // Hex format
            let hex = &parts[0][2..]; // Remove "0x" prefix
            let vec = Vector::from_hex(hex)?;
            let repeat = parse_repeat(&parts)?;
            Ok(ParsedLine::Vector(vec, repeat))
        }

        s if is_binary_string(s) => {
            // Binary format (starts with 0 or 1)
            let vec = Vector::from_binary(parts[0])?;
            let repeat = parse_repeat(&parts)?;
            Ok(ParsedLine::Vector(vec, repeat))
        }

        _ => Err(format!("Unknown directive: {}", parts[0])),
    }
}

/// Check if string looks like a binary vector
fn is_binary_string(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| matches!(c, '0' | '1' | 'X' | 'Z' | 'H' | 'L' | '-'))
}

/// Parse repeat count from parts (e.g., "REPEAT 1000")
fn parse_repeat(parts: &[&str]) -> Result<u32, String> {
    for i in 0..parts.len() {
        if parts[i].to_uppercase() == "REPEAT" {
            if i + 1 >= parts.len() {
                return Err("REPEAT requires a count".into());
            }
            return parts[i + 1]
                .replace("_", "")
                .parse::<u32>()
                .map_err(|e| format!("Invalid repeat count: {}", e));
        }
    }
    Ok(1) // Default repeat count
}

/// Parse pin type string
fn parse_pin_type(s: &str) -> Result<PinType, String> {
    match s.to_uppercase().as_str() {
        "BIDI" | "BIDIRECTIONAL" | "IO" => Ok(PinType::Bidi),
        "INPUT" | "IN" | "I" => Ok(PinType::Input),
        "OUTPUT" | "OUT" | "O" => Ok(PinType::Output),
        "OC" | "OPENCOLLECTOR" | "OPEN_COLLECTOR" => Ok(PinType::OpenCollector),
        "PULSE" | "P" => Ok(PinType::Pulse),
        "NPULSE" | "NP" => Ok(PinType::NPulse),
        "ERROR" | "ERRORTRIG" | "ERROR_TRIG" => Ok(PinType::ErrorTrig),
        "VECCLK" | "VEC_CLK" | "CLOCK" => Ok(PinType::VecClk),
        "VECCLKEN" | "VEC_CLK_EN" | "CLOCKEN" => Ok(PinType::VecClkEn),
        _ => Err(format!("Unknown pin type: {}", s)),
    }
}

/// FVEC parsing error
#[derive(Debug)]
pub enum FvecError {
    Io(std::io::Error),
    Parse { line: usize, msg: String },
}

impl std::fmt::Display for FvecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FvecError::Io(e) => write!(f, "I/O error: {}", e),
            FvecError::Parse { line, msg } => write!(f, "Parse error at line {}: {}", line, msg),
        }
    }
}

impl std::error::Error for FvecError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_basic() {
        let fvec = r#"
            # Test file
            CLOCK 50_000_000

            PIN 0 INPUT
            PIN 1-7 OUTPUT

            ZERO
            ONES REPEAT 10
        "#;

        let program = FvecProgram::from_str(fvec).unwrap();
        assert_eq!(program.clock_hz, 50_000_000);
        assert_eq!(program.pin_config.types[0], PinType::Input);
        assert_eq!(program.pin_config.types[1], PinType::Output);
        assert_eq!(program.pin_config.types[7], PinType::Output);
        assert_eq!(program.vectors.len(), 2);
        assert_eq!(program.vectors[0].vector, Vector::ZERO);
        assert_eq!(program.vectors[0].repeat, 1);
        assert_eq!(program.vectors[1].vector, Vector::ONES);
        assert_eq!(program.vectors[1].repeat, 10);
    }

    #[test]
    fn test_parse_toggle() {
        let fvec = r#"
            ZERO
            TOGGLE 0 1 2
            TOGGLE 0
        "#;

        let program = FvecProgram::from_str(fvec).unwrap();
        assert_eq!(program.vectors.len(), 3);

        // First: all zeros
        assert!(!program.vectors[0].vector.get_bit(0));

        // Second: bits 0,1,2 toggled on
        assert!(program.vectors[1].vector.get_bit(0));
        assert!(program.vectors[1].vector.get_bit(1));
        assert!(program.vectors[1].vector.get_bit(2));
        assert!(!program.vectors[1].vector.get_bit(3));

        // Third: bit 0 toggled off
        assert!(!program.vectors[2].vector.get_bit(0));
        assert!(program.vectors[2].vector.get_bit(1));
        assert!(program.vectors[2].vector.get_bit(2));
    }

    #[test]
    fn test_parse_binary() {
        // 160 chars of alternating 1s and 0s
        let pattern: String = (0..160).map(|i| if i % 2 == 0 { '1' } else { '0' }).collect();
        let fvec = format!("{} REPEAT 5", pattern);

        let program = FvecProgram::from_str(&fvec).unwrap();
        assert_eq!(program.vectors.len(), 1);
        assert_eq!(program.vectors[0].repeat, 5);
        assert!(program.vectors[0].vector.get_bit(0));
        assert!(!program.vectors[0].vector.get_bit(1));
    }

    #[test]
    fn test_parse_hex() {
        let fvec = "0x0102030405060708090a0b0c0d0e0f1011121314";
        let program = FvecProgram::from_str(fvec).unwrap();
        assert_eq!(program.vectors.len(), 1);
        assert_eq!(program.vectors[0].vector.data[0], 0x01);
        assert_eq!(program.vectors[0].vector.data[19], 0x14);
    }

    #[test]
    fn test_total_vectors() {
        let fvec = r#"
            ZERO REPEAT 100
            ONES REPEAT 200
            ZERO REPEAT 50
        "#;

        let program = FvecProgram::from_str(fvec).unwrap();
        assert_eq!(program.total_vectors(), 350);
    }
}
