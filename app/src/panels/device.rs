/// Device configuration panel — replaces Unity's .bim editor
/// Tabs: Pin Map | Signal Map | Levels | Timing | Test Plan
/// Generates PIN_MAP, .map, .lvl, .tim, .tp, PowerOn.sh, PowerOff.sh

use crate::ui::Ui;
use crate::layout::{Rect, Column, Row};
use crate::state::AppState;
use crate::theme;

const PROFILES: [&str; 4] = ["Sonoma", "HX", "XP-160", "MCC"];

pub fn draw(ui: &mut Ui, rect: Rect, _state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    // --- Header row: title + profile badge ---
    let header = col.next(48.0);
    let mut hdr_row = Row::new(header).with_gap(12.0);
    let title_r = hdr_row.next(280.0);
    ui.label(
        title_r.x,
        title_r.y + 8.0,
        "Device Configuration",
        theme::FONT_SIZE_TITLE,
        theme::TEXT_PRIMARY,
    );
    let profile_idx = _state.tab_index("device_profile");
    let profile_name = PROFILES[profile_idx.min(PROFILES.len() - 1)];
    let badge_r = hdr_row.next(100.0);
    ui.badge(badge_r.x, badge_r.y + 4.0, profile_name, theme::ACCENT);

    // --- Action buttons row ---
    let btn_row = col.next(40.0);
    let mut row = Row::new(btn_row).with_gap(8.0);
    if ui.button_ghost(row.next(140.0), "Load Device JSON") {
        _state.status_message = "File dialog: load device JSON".into();
    }
    if ui.button(row.next(120.0), "Generate All") {
        _state.status_message = format!("Generate all files for profile: {}", profile_name);
    }

    // Profile selector buttons
    let _spacer = row.next(24.0);
    ui.label(
        row.next(50.0).x,
        btn_row.y + 10.0,
        "Profile:",
        theme::FONT_SIZE_SMALL,
        theme::TEXT_SECONDARY,
    );
    for (i, name) in PROFILES.iter().enumerate() {
        let r = row.next(72.0);
        if i == profile_idx {
            if ui.button(r, name) {
                // Already selected
            }
        } else if ui.button_ghost(r, name) {
            _state.set_tab_index("device_profile", i);
        }
    }

    // --- Section tabs ---
    let tab_r = col.next(36.0);
    let tab_labels = ["Pin Map", "Signal Map", "Levels", "Timing", "Test Plan"];
    let active_tab = _state.tab_index("device");
    if let Some(idx) = ui.tabs(tab_r, &tab_labels, active_tab) {
        _state.set_tab_index("device", idx);
    }
    let active_tab = _state.tab_index("device");

    // --- Tab body ---
    let body = col.remaining();

    match active_tab {
        0 => draw_pin_map(ui, body, _state),
        1 => draw_signal_map(ui, body, _state),
        2 => draw_levels(ui, body, _state),
        3 => draw_timing(ui, body, _state),
        4 => draw_test_plan(ui, body, _state),
        _ => {}
    }
}

/// Pin Map tab — GPIO index to signal name mapping (PIN_MAP file)
fn draw_pin_map(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "Pin Map (PIN_MAP)");
    let mut col = Column::new(inner).with_gap(8.0);

    let r = col.next(24.0);
    ui.label(
        r.x,
        r.y,
        "GPIO index to signal assignment — 128 BIM + 32 fast pins",
        theme::FONT_SIZE_NORMAL,
        theme::TEXT_SECONDARY,
    );

    let widths = [60.0, 160.0, 80.0, 80.0];
    let hdr_r = col.next(theme::ROW_HEIGHT);
    ui.table_header(hdr_r, &["GPIO", "Signal", "Direction", "Type"], &widths);

    // Placeholder rows — BIM pins 0-7
    let placeholder_pins: [(u32, &str, &str, &str); 8] = [
        (0, "(unassigned)", "BIDI", "IO"),
        (1, "(unassigned)", "BIDI", "IO"),
        (2, "(unassigned)", "BIDI", "IO"),
        (3, "(unassigned)", "BIDI", "IO"),
        (4, "(unassigned)", "BIDI", "IO"),
        (5, "(unassigned)", "BIDI", "IO"),
        (6, "(unassigned)", "BIDI", "IO"),
        (7, "(unassigned)", "BIDI", "IO"),
    ];

    for (i, (gpio, signal, dir, ptype)) in placeholder_pins.iter().enumerate() {
        let r = col.next(theme::ROW_HEIGHT);
        ui.table_row(
            r,
            &[&format!("{}", gpio), signal, dir, ptype],
            &widths,
            i % 2 == 0,
        );
    }

    let r = col.next(28.0);
    ui.label(
        r.x + 12.0,
        r.y + 8.0,
        "... 152 more pins — load device JSON to populate",
        theme::FONT_SIZE_SMALL,
        theme::TEXT_DISABLED,
    );
}

/// Signal Map tab — signal to bank/GPIO# assignment (.map file)
fn draw_signal_map(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "Signal Map (.map)");
    let mut col = Column::new(inner).with_gap(8.0);

    let r = col.next(24.0);
    ui.label(
        r.x,
        r.y,
        "Signal = BANK_GPIO# ; DIR — from .map file format",
        theme::FONT_SIZE_NORMAL,
        theme::TEXT_SECONDARY,
    );

    let widths = [160.0, 80.0, 80.0, 80.0];
    let hdr_r = col.next(theme::ROW_HEIGHT);
    ui.table_header(hdr_r, &["Signal", "Bank", "GPIO#", "Direction"], &widths);

    // Placeholder rows
    let placeholder: [(&str, &str, &str, &str); 6] = [
        ("PAD_A_RSTN", "B13", "0", "OUTPUT"),
        ("PAD_A_CLK", "B13", "1", "PULSE"),
        ("PAD_A_TDI", "B13", "2", "INPUT"),
        ("PAD_A_TDO", "B13", "3", "MONITOR"),
        ("PAD_A_TMS", "B13", "4", "INPUT"),
        ("PAD_A_TCK", "B13", "5", "PULSE"),
    ];

    for (i, (signal, bank, gpio, dir)) in placeholder.iter().enumerate() {
        let r = col.next(theme::ROW_HEIGHT);
        ui.table_row(r, &[signal, bank, gpio, dir], &widths, i % 2 == 0);
    }

    let r = col.next(28.0);
    ui.label(
        r.x + 12.0,
        r.y + 8.0,
        "Load device JSON to populate full signal map",
        theme::FONT_SIZE_SMALL,
        theme::TEXT_DISABLED,
    );
}

/// Levels tab — bank voltage table with derived CMOS levels (.lvl file)
fn draw_levels(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "Bank Voltages (.lvl)");
    let mut col = Column::new(inner).with_gap(8.0);

    let r = col.next(24.0);
    ui.label(
        r.x,
        r.y,
        "CMOS 70/30 rule: VIH=0.7V, VIL=0.3V, VOH=0.8V, VOL=0.2V",
        theme::FONT_SIZE_NORMAL,
        theme::TEXT_SECONDARY,
    );

    let widths = [60.0, 80.0, 80.0, 80.0, 80.0, 80.0];
    let hdr_r = col.next(theme::ROW_HEIGHT);
    ui.table_header(
        hdr_r,
        &["Bank", "Voltage", "VIH", "VIL", "VOH", "VOL"],
        &widths,
    );

    // 4 IO banks — placeholder voltages
    let banks: [(&str, f32); 4] = [
        ("B13", 1.800),
        ("B33", 1.800),
        ("B34", 1.800),
        ("B35", 1.800),
    ];

    for (i, (bank, voltage)) in banks.iter().enumerate() {
        let vih = format!("{:.3}", voltage * 0.7);
        let vil = format!("{:.3}", voltage * 0.3);
        let voh = format!("{:.3}", voltage * 0.8);
        let vol = format!("{:.3}", voltage * 0.2);
        let r = col.next(theme::ROW_HEIGHT);
        ui.table_row(
            r,
            &[bank, &format!("{:.3}", voltage), &vih, &vil, &voh, &vol],
            &widths,
            i % 2 == 0,
        );
    }

    let r = col.next(28.0);
    ui.label(
        r.x,
        r.y + 8.0,
        "Edit voltages in device JSON — levels derived automatically",
        theme::FONT_SIZE_SMALL,
        theme::TEXT_DISABLED,
    );
}

/// Timing tab — per-pin timing parameters (.tim file)
fn draw_timing(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "Timing Parameters (.tim)");
    let mut col = Column::new(inner).with_gap(8.0);

    let r = col.next(24.0);
    ui.label(
        r.x,
        r.y,
        "PERIOD, DRIVE_OFF, COMPARE — from .tim file format",
        theme::FONT_SIZE_NORMAL,
        theme::TEXT_SECONDARY,
    );

    let widths = [80.0, 80.0, 80.0, 80.0, 80.0, 80.0];
    let hdr_r = col.next(theme::ROW_HEIGHT);
    ui.table_header(
        hdr_r,
        &["Pin", "Type", "Period", "Rise", "Fall", "Compare"],
        &widths,
    );

    // Placeholder timing entries
    let timing: [(&str, &str, &str, &str, &str, &str); 4] = [
        ("CLK", "PULSE_POS", "10 ns", "1 ns", "1 ns", "5 ns"),
        ("IO_0", "BIDI", "10 ns", "1 ns", "1 ns", "8 ns"),
        ("TDO", "MONITOR", "10 ns", "-", "-", "8 ns"),
        ("RSTN", "OUTPUT", "10 ns", "1 ns", "1 ns", "-"),
    ];

    for (i, (pin, ptype, period, rise, fall, compare)) in timing.iter().enumerate() {
        let r = col.next(theme::ROW_HEIGHT);
        ui.table_row(r, &[pin, ptype, period, rise, fall, compare], &widths, i % 2 == 0);
    }

    let r = col.next(28.0);
    ui.label(
        r.x + 12.0,
        r.y + 8.0,
        "Load device JSON to populate timing from .tim definition",
        theme::FONT_SIZE_SMALL,
        theme::TEXT_DISABLED,
    );
}

/// Test Plan tab — step sequence from .tp file
fn draw_test_plan(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "Test Plan Steps (.tp)");
    let mut col = Column::new(inner).with_gap(8.0);

    let r = col.next(24.0);
    ui.label(
        r.x,
        r.y,
        "STEP PATTERN PATTERN_FILE LOOPS — from .tp file format",
        theme::FONT_SIZE_NORMAL,
        theme::TEXT_SECONDARY,
    );

    let widths = [60.0, 140.0, 60.0, 80.0, 80.0];
    let hdr_r = col.next(theme::ROW_HEIGHT);
    ui.table_header(
        hdr_r,
        &["Step#", "Pattern", "Loops", "Temp", "Duration"],
        &widths,
    );

    // Placeholder steps
    let steps: [(&str, &str, &str, &str, &str); 3] = [
        ("1", "bringup_fast_pins.fbc", "1", "25 C", "00:01:00"),
        ("2", "checkerboard.fbc", "100", "125 C", "01:00:00"),
        ("3", "walking_ones.fbc", "1000", "125 C", "04:00:00"),
    ];

    for (i, (step, pattern, loops, temp, duration)) in steps.iter().enumerate() {
        let r = col.next(theme::ROW_HEIGHT);
        ui.table_row(r, &[step, pattern, loops, temp, duration], &widths, i % 2 == 0);
    }

    let r = col.next(28.0);
    ui.label(
        r.x + 12.0,
        r.y + 8.0,
        "Load device JSON to populate test plan steps",
        theme::FONT_SIZE_SMALL,
        theme::TEXT_DISABLED,
    );
}
