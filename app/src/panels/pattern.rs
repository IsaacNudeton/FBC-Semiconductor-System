/// Pattern converter panel — 3 tabs matching the C pattern converter
/// Tab 1: Pattern Conversion (ATP/STIL/AVC -> .hex/.fbc)
/// Tab 2: Device Config (JSON -> 7 device files)
/// Tab 3: Pin Import (CSV/Excel/PDF -> editable table -> device files)

use crate::ui::Ui;
use crate::layout::{Rect, Column, Row};
use crate::state::AppState;
use crate::theme;
use crate::pattern_converter::{PcHandle, DcHandle};

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Pattern Converter", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    // 3 tabs matching C converter pipelines
    let tab_r = col.next(36.0);
    let tab_labels = ["Pattern Convert", "Device Config", "Pin Import"];
    let active_tab = state.tab_index("pattern");
    if let Some(idx) = ui.tabs(tab_r, &tab_labels, active_tab) {
        state.set_tab_index("pattern", idx);
    }
    let active_tab = state.tab_index("pattern");

    let body = col.remaining();

    match active_tab {
        0 => draw_convert_tab(ui, body, state),
        1 => draw_device_config_tab(ui, body, state),
        2 => draw_pin_import_tab(ui, body, state),
        _ => {}
    }
}

fn draw_convert_tab(ui: &mut Ui, body: Rect, state: &mut AppState) {
    let inner = ui.card(body, "Pattern Conversion");
    let mut icol = Column::new(inner).with_gap(10.0);

    // Input file
    let r = icol.next(28.0);
    ui.label(r.x, r.y + 4.0, "Input:", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
    let btn_r = Rect::new(r.x + 60.0, r.y, 160.0, 28.0);
    if ui.button_ghost(btn_r, "Select ATP/STIL/AVC") {
        if let Some(path) = pick_file(&["atp", "stil", "avc", "txt"]) {
            state.status_message = format!("Input: {}", path);
        }
    }

    // Pin map (optional)
    let r = icol.next(28.0);
    ui.label(r.x, r.y + 4.0, "Pin Map:", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
    let btn_r = Rect::new(r.x + 60.0, r.y, 160.0, 28.0);
    if ui.button_ghost(btn_r, "Select PIN_MAP") {
        if let Some(path) = pick_file(&["txt", "pin", "map", "*"]) {
            state.status_message = format!("Pin map: {}", path);
        }
    }

    // Format selector
    let r = icol.next(36.0);
    let fmt_labels = [".hex", ".fbc", ".hex + .fbc"];
    let fmt_active = state.tab_index("pattern_fmt");
    if let Some(idx) = ui.tabs(r, &fmt_labels, fmt_active) {
        state.set_tab_index("pattern_fmt", idx);
    }
    let fmt_active = state.tab_index("pattern_fmt");

    // Vec clock Hz (for .fbc)
    if fmt_active >= 1 {
        let r = icol.next(24.0);
        ui.label(r.x, r.y, "Vec clock: 5000000 Hz (default)", theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
    }

    // Convert button
    let r = icol.next(40.0);
    if ui.button(Rect::new(r.x, r.y, 120.0, 36.0), "Convert") {
        run_convert(state, fmt_active);
    }

    // Output info
    let r = icol.next(24.0);
    ui.label(r.x, r.y, "Output: files written to same directory as input", theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    // Stats from last conversion
    let r = icol.next(20.0);
    ui.label(r.x, r.y, &state.status_message, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
}

fn draw_device_config_tab(ui: &mut Ui, body: Rect, state: &mut AppState) {
    let inner = ui.card(body, "Device Config Generation");
    let mut icol = Column::new(inner).with_gap(10.0);

    // Device JSON
    let r = icol.next(28.0);
    ui.label(r.x, r.y + 4.0, "Device JSON:", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
    let btn_r = Rect::new(r.x + 100.0, r.y, 160.0, 28.0);
    if ui.button_ghost(btn_r, "Select device.json") {
        if let Some(path) = pick_file(&["json"]) {
            state.status_message = format!("Device: {}", path);
        }
    }

    // Profile dropdown
    let r = icol.next(36.0);
    let profiles = ["Sonoma", "HX", "XP-160/Shasta", "MCC"];
    let profile_active = state.tab_index("dc_profile");
    if let Some(idx) = ui.tabs(r, &profiles, profile_active) {
        state.set_tab_index("dc_profile", idx);
    }
    let profile_active = state.tab_index("dc_profile");

    // Output dir
    let r = icol.next(28.0);
    ui.label(r.x, r.y + 4.0, "Output Dir:", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
    let btn_r = Rect::new(r.x + 100.0, r.y, 160.0, 28.0);
    if ui.button_ghost(btn_r, "Select directory") {
        if let Some(path) = pick_folder() {
            state.status_message = format!("Output: {}", path);
        }
    }

    // Generate button
    let r = icol.next(40.0);
    if ui.button(Rect::new(r.x, r.y, 120.0, 36.0), "Generate") {
        run_device_gen(state, profile_active);
    }

    // Output files list
    let r = icol.next(24.0);
    ui.label(r.x, r.y, "Generates: PIN_MAP, .map, .lvl, .tim, .tp, PowerOn.sh, PowerOff.sh", theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
}

fn draw_pin_import_tab(ui: &mut Ui, body: Rect, state: &mut AppState) {
    let inner = ui.card(body, "Pin Import & Verification");
    let mut icol = Column::new(inner).with_gap(10.0);

    // Primary source
    let r = icol.next(28.0);
    ui.label(r.x, r.y + 4.0, "Primary:", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
    let btn_r = Rect::new(r.x + 80.0, r.y, 180.0, 28.0);
    if ui.button_ghost(btn_r, "Select CSV/Excel/PDF") {
        if let Some(path) = pick_file(&["csv", "xlsx", "xls", "pdf"]) {
            state.status_message = format!("Primary: {}", path);
        }
    }

    // Secondary source (for cross-verify)
    let r = icol.next(28.0);
    ui.label(r.x, r.y + 4.0, "Verify:", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
    let btn_r = Rect::new(r.x + 80.0, r.y, 180.0, 28.0);
    if ui.button_ghost(btn_r, "Select 2nd source") {
        if let Some(path) = pick_file(&["csv", "xlsx", "xls", "pdf"]) {
            state.status_message = format!("Secondary: {}", path);
        }
    }

    // Extract + Verify + Generate buttons
    let r = icol.next(40.0);
    let mut btn_row = Row::new(r).with_gap(8.0);
    if ui.button(btn_row.next(100.0), "Extract") {
        state.status_message = "Extract pin table from file".into();
    }
    if ui.button_ghost(btn_row.next(120.0), "Cross-Verify") {
        state.status_message = "Compare primary vs secondary pin tables".into();
    }
    if ui.button(btn_row.next(100.0), "Generate") {
        state.status_message = "Generate device files from extracted pins".into();
    }

    // Extracted table display
    let table_r = icol.next(200.0);
    let widths = [80.0, 120.0, 80.0, 80.0, 80.0];
    let hdr_r = Rect::new(table_r.x, table_r.y, table_r.w, theme::ROW_HEIGHT);
    ui.table_header(hdr_r, &["Channel", "Signal", "Direction", "Voltage", "Group"], &widths);

    let r = Rect::new(table_r.x, table_r.y + theme::ROW_HEIGHT + 8.0, table_r.w, 20.0);
    ui.label(r.x + 12.0, r.y, "Extract a pin table to populate this view", theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
}

// ---- File dialog helpers (rfd) ----

fn pick_file(extensions: &[&str]) -> Option<String> {
    let mut dialog = rfd::FileDialog::new();
    for ext in extensions {
        dialog = dialog.add_filter(ext.to_uppercase(), &[ext]);
    }
    dialog.pick_file().map(|p| p.display().to_string())
}

fn pick_folder() -> Option<String> {
    rfd::FileDialog::new().pick_folder().map(|p| p.display().to_string())
}

// ---- C Engine integration ----

fn run_convert(state: &mut AppState, fmt: usize) {
    let handle = match PcHandle::new() {
        Ok(h) => h,
        Err(e) => { state.status_message = format!("Engine error: {}", e); return; }
    };

    // TODO: wire to actual file paths from state
    // For now, show that the engine is functional
    state.status_message = format!(
        "C Engine v{} ready. Select input file and click Convert. Format: {}",
        PcHandle::version(),
        match fmt { 0 => ".hex", 1 => ".fbc", _ => ".hex + .fbc" },
    );
    drop(handle);
}

fn run_device_gen(state: &mut AppState, profile_idx: usize) {
    let profiles = ["sonoma", "hx", "xp-160", "mcc"];
    let profile = profiles[profile_idx.min(profiles.len() - 1)];

    let handle = match DcHandle::new() {
        Ok(h) => h,
        Err(e) => { state.status_message = format!("DC error: {}", e); return; }
    };

    match handle.load_profile(profile) {
        Ok(()) => {
            state.status_message = format!("Profile '{}' loaded. Select device JSON and output dir.", profile);
        }
        Err(e) => {
            state.status_message = format!("Profile error: {}", e);
        }
    }
}
