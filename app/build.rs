/// Build script — compiles the C pattern converter engine into libpattern_converter.a
///
/// Sources: gui/src-tauri/c-engine/pc/ (shared with Tauri GUI)
/// Output: linked into fbc-app binary

fn main() {
    let pc_dir = "../gui/src-tauri/c-engine/pc";

    cc::Build::new()
        .files([
            format!("{}/ir.c", pc_dir),
            format!("{}/crc32.c", pc_dir),
            format!("{}/parse_atp.c", pc_dir),
            format!("{}/parse_pinmap.c", pc_dir),
            format!("{}/parse_stil_smart.c", pc_dir),
            format!("{}/parse_avc_smart.c", pc_dir),
            format!("{}/gen_hex.c", pc_dir),
            format!("{}/gen_seq.c", pc_dir),
            format!("{}/gen_fbc.c", pc_dir),
            format!("{}/dc_json.c", pc_dir),
            format!("{}/dc_gen.c", pc_dir),
            format!("{}/dc_csv.c", pc_dir),
            format!("{}/dll_api.c", pc_dir),
            format!("{}/dc_api.c", pc_dir),
            format!("{}/vendor/cJSON.c", pc_dir),
        ])
        .include(pc_dir)
        .define("_CRT_SECURE_NO_WARNINGS", None)
        .warnings(false)
        .compile("pattern_converter");

    println!("cargo:rerun-if-changed={}", pc_dir);
}
