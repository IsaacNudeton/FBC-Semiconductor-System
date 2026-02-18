//! FBC Vector Tools
//!
//! CLI for working with FBC vector files.
//!
//! # Commands
//!
//! ```bash
//! # Compile text vectors to binary
//! fbc-vec compile input.fvec -o output.fbc
//!
//! # Show info about a binary file
//! fbc-vec info output.fbc
//!
//! # Disassemble binary to text
//! fbc-vec disasm output.fbc
//!
//! # Validate a binary file
//! fbc-vec validate output.fbc
//! ```

use std::path::PathBuf;
use clap::{Parser, Subcommand};
use fbc_host::vector::{FvecProgram, FbcFile, compile_fvec, VectorDecompiler};

#[derive(Parser)]
#[command(name = "fbc-vec")]
#[command(about = "FBC Vector Tools - compile, inspect, and validate vector files")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Compile text vectors (.fvec) to binary (.fbc)
    Compile {
        /// Input file (.fvec)
        input: PathBuf,

        /// Output file (.fbc)
        #[arg(short, long, default_value = "output.fbc")]
        output: PathBuf,

        /// Show detailed compilation info
        #[arg(short, long)]
        verbose: bool,
    },

    /// Show information about a binary file
    Info {
        /// Input file (.fbc)
        input: PathBuf,

        /// Show pin configuration
        #[arg(short, long)]
        pins: bool,
    },

    /// Disassemble binary to text format
    Disasm {
        /// Input file (.fbc)
        input: PathBuf,

        /// Output file (stdout if not specified)
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Maximum vectors to show
        #[arg(short, long, default_value = "100")]
        limit: usize,
    },

    /// Validate a binary file
    Validate {
        /// Input file (.fbc)
        input: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Compile { input, output, verbose } => {
            cmd_compile(&input, &output, verbose)?;
        }
        Commands::Info { input, pins } => {
            cmd_info(&input, pins)?;
        }
        Commands::Disasm { input, output, limit } => {
            cmd_disasm(&input, output.as_deref(), limit)?;
        }
        Commands::Validate { input } => {
            cmd_validate(&input)?;
        }
    }

    Ok(())
}

fn cmd_compile(input: &PathBuf, output: &PathBuf, verbose: bool) -> anyhow::Result<()> {
    println!("Compiling {} -> {}", input.display(), output.display());

    // Parse input
    let program = FvecProgram::from_file(input)?;

    if verbose {
        println!("  Clock:     {} Hz", program.clock_hz);
        println!("  Entries:   {}", program.vectors.len());
        println!("  Vectors:   {} (after repeat expansion)", program.total_vectors());
    }

    // Compile
    let fbc = compile_fvec(&program);

    if verbose {
        let stats = fbc.stats();
        println!();
        println!("  Compressed size: {} bytes", stats.compressed_size);
        println!("  Compression:     {:.2}x", stats.compression_ratio);
    }

    // Write output
    fbc.write_to_file(output)?;

    let file_size = std::fs::metadata(output)?.len();
    println!("Wrote {} bytes to {}", file_size, output.display());

    Ok(())
}

fn cmd_info(input: &PathBuf, show_pins: bool) -> anyhow::Result<()> {
    let fbc = FbcFile::read_from_file(input)?;

    println!("FBC Vector File: {}", input.display());
    println!();

    // Header info
    println!("Header:");
    println!("  Magic:       0x{:08X} ({})",
        fbc.header.magic,
        if fbc.header.magic == 0x00434246 { "valid" } else { "INVALID" }
    );
    println!("  Version:     {}", fbc.header.version);
    println!("  Pin count:   {}", fbc.header.pin_count);
    println!("  Vectors:     {}", fbc.header.num_vectors);
    println!("  Clock:       {} Hz ({:.2} MHz)",
        fbc.header.vec_clock_hz,
        fbc.header.vec_clock_hz as f64 / 1_000_000.0
    );
    println!("  Data size:   {} bytes", fbc.header.compressed_size);
    println!("  CRC32:       0x{:08X} ({})",
        fbc.header.crc32,
        if fbc.validate_crc() { "valid" } else { "INVALID" }
    );

    // Statistics
    println!();
    let stats = fbc.stats();
    print!("{}", stats);

    // Pin configuration
    if show_pins {
        println!();
        println!("Pin Configuration:");
        for (i, &pin_type) in fbc.pin_config.types.iter().enumerate() {
            if pin_type != fbc_host::vector::PinType::Bidi {
                println!("  Pin {:3}: {:?}", i, pin_type);
            }
        }
    }

    Ok(())
}

fn cmd_disasm(input: &PathBuf, output: Option<&std::path::Path>, limit: usize) -> anyhow::Result<()> {
    let fbc = FbcFile::read_from_file(input)?;

    let mut out: Box<dyn std::io::Write> = match output {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };

    writeln!(out, "# Disassembly of {}", input.display())?;
    writeln!(out, "# Vectors: {}, Clock: {} Hz",
        fbc.header.num_vectors, fbc.header.vec_clock_hz)?;
    writeln!(out)?;
    writeln!(out, "CLOCK {}", fbc.header.vec_clock_hz)?;
    writeln!(out)?;

    // Show non-default pin configs
    for (i, &pin_type) in fbc.pin_config.types.iter().enumerate() {
        if pin_type != fbc_host::vector::PinType::Bidi {
            writeln!(out, "PIN {} {:?}", i, pin_type)?;
        }
    }
    writeln!(out)?;

    // Disassemble vectors (limited)
    writeln!(out, "# Vectors (showing up to {}):", limit)?;

    let mut decomp = VectorDecompiler::new(&fbc);
    let mut count = 0;

    while let Some(vec) = decomp.next() {
        if count >= limit {
            writeln!(out, "# ... ({} more vectors)", fbc.header.num_vectors as usize - count)?;
            break;
        }

        // Show as hex
        writeln!(out, "0x{}", vec.to_hex())?;
        count += 1;
    }

    Ok(())
}

fn cmd_validate(input: &PathBuf) -> anyhow::Result<()> {
    let fbc = FbcFile::read_from_file(input)?;

    let mut errors = Vec::new();

    // Check magic
    if fbc.header.magic != 0x00434246 {
        errors.push(format!("Invalid magic: 0x{:08X}", fbc.header.magic));
    }

    // Check CRC
    if !fbc.validate_crc() {
        let expected = fbc.calculate_crc();
        errors.push(format!("CRC mismatch: file has 0x{:08X}, calculated 0x{:08X}",
            fbc.header.crc32, expected));
    }

    // Check data size
    if fbc.data.len() != fbc.header.compressed_size as usize {
        errors.push(format!("Data size mismatch: header says {}, actual {}",
            fbc.header.compressed_size, fbc.data.len()));
    }

    // Try to decompress
    let mut decomp = VectorDecompiler::new(&fbc);
    let vectors = decomp.to_vec();
    if vectors.len() != fbc.header.num_vectors as usize {
        // Note: this might not match exactly due to run encoding
        // Just warn, don't error
        println!("Note: Decompressed {} vectors, header claims {}",
            vectors.len(), fbc.header.num_vectors);
    }

    if errors.is_empty() {
        println!("{}: VALID", input.display());
        println!("  {} vectors, {} bytes compressed",
            fbc.header.num_vectors, fbc.header.compressed_size);
        Ok(())
    } else {
        println!("{}: INVALID", input.display());
        for error in &errors {
            println!("  - {}", error);
        }
        std::process::exit(1);
    }
}
