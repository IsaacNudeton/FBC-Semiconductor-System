/*
 * build.rs — Tauri build + C engine compilation
 *
 * 1. Runs tauri_build::build() for Tauri scaffolding
 * 2. Compiles the LRM C database engine (c-engine/)
 * 3. Compiles the Pattern Converter C library (c-engine/pc/)
 */

use std::path::PathBuf;

fn main() {
    /* Tauri build scaffolding (required) */
    tauri_build::build();

    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let c_engine = manifest_dir.join("c-engine");

    if !c_engine.exists() {
        println!("cargo:warning=c-engine/ not found, skipping C compilation");
        return;
    }

    /* ═══════════════════════════════════════════════════════════
     * 1. LRM C Database Engine
     * ═══════════════════════════════════════════════════════════ */
    {
        let lrm_sources = [
            "page.c", "btree.c", "wal.c", "table.c",
            "schema.c", "lrm_ffi.c",
            // inventory.c and tracker.c excluded — they define lrm_get_lot/lrm_advance_lot
            // with Database* signatures that conflict with the LrmDatabase* FFI wrappers
        ];

        let mut build = cc::Build::new();
        build
            .std("c11")
            .warnings(false)
            .opt_level(2)
            .include(&c_engine)
            .define("_CRT_SECURE_NO_WARNINGS", None);

        let mut found = false;
        for src in &lrm_sources {
            let path = c_engine.join(src);
            if path.exists() {
                build.file(&path);
                println!("cargo:rerun-if-changed={}", path.display());
                found = true;
            }
        }

        if found {
            build.compile("lrm_db");
        }
    }

    /* ═══════════════════════════════════════════════════════════
     * 2. Pattern Converter (pc + dc pipelines)
     * ═══════════════════════════════════════════════════════════ */
    {
        let pc_dir = c_engine.join("pc");
        if !pc_dir.exists() {
            println!("cargo:warning=c-engine/pc/ not found, skipping pattern converter");
            return;
        }

        let pc_sources = [
            /* Core IR + CRC */
            "ir.c", "crc32.c",
            /* Parsers */
            "parse_atp.c", "parse_pinmap.c",
            "parse_stil_smart.c", "parse_avc_smart.c",
            /* Generators */
            "gen_hex.c", "gen_seq.c", "gen_fbc.c",
            /* Device config pipeline */
            "dc_json.c", "dc_gen.c", "dc_csv.c",
            /* DLL APIs (handle-based FFI) */
            "dll_api.c", "dc_api.c",
            /* Vendored cJSON */
            "vendor/cJSON.c",
        ];

        let mut build = cc::Build::new();
        build
            .std("c11")
            .warnings(false)
            .opt_level(2)
            .include(&pc_dir)
            .include(&pc_dir.join("vendor"))
            .define("_CRT_SECURE_NO_WARNINGS", None)
            .define("PC_EXPORT", None);

        let mut found = false;
        for src in &pc_sources {
            let path = pc_dir.join(src);
            if path.exists() {
                build.file(&path);
                println!("cargo:rerun-if-changed={}", path.display());
                found = true;
            }
        }

        if found {
            build.compile("pattern_converter");
        }
    }
}
