/// Test plan panel — TestConfig JSON editor with sections
/// Tabs: General | Power | Timing | Vectors | Thermal

use crate::ui::Ui;
use crate::layout::{Rect, Column, Row};
use crate::state::AppState;
use crate::theme;

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Test Plan", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    // Load/Save buttons
    let btn_row = col.next(40.0);
    let mut row = Row::new(btn_row).with_gap(8.0);
    if ui.button_ghost(row.next(100.0), "Load JSON") {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .add_filter("Test Plan", &["tpf"])
            .pick_file()
        {
            state.status_message = format!("Loaded: {}", path.display());
        }
    }
    if ui.button_ghost(row.next(100.0), "Save JSON") {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("JSON", &["json"])
            .set_file_name("testplan.json")
            .save_file()
        {
            state.status_message = format!("Saved: {}", path.display());
        }
    }
    if ui.button(row.next(100.0), "New") {
        state.status_message = "Created new test plan".into();
    }

    // Section tabs
    let tab_r = col.next(36.0);
    let tab_labels = ["General", "Power", "Timing", "Vectors", "Thermal"];
    let active_tab = state.tab_index("testplan");
    if let Some(idx) = ui.tabs(tab_r, &tab_labels, active_tab) {
        state.set_tab_index("testplan", idx);
    }
    let active_tab = state.tab_index("testplan");

    let body = col.remaining();

    match active_tab {
        0 => draw_general(ui, body, state),
        1 => draw_power(ui, body, state),
        2 => draw_timing(ui, body, state),
        3 => draw_vectors(ui, body, state),
        4 => draw_thermal(ui, body, state),
        _ => {}
    }
}

fn draw_general(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "General Configuration");
    let mut col = Column::new(inner).with_gap(8.0);

    let fields = [
        ("Device Dir", "Path to device directory on board"),
        ("Seq Path", "Path to .seq file"),
        ("Hex Path", "Path to .hex file"),
        ("Frequency", "Vector clock frequency (Hz)"),
        ("Duration", "Run time (seconds)"),
    ];

    for (label, hint) in &fields {
        let r = col.next(32.0);
        ui.label(r.x, r.y + 8.0, label, theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
        ui.label(r.x + 120.0, r.y + 8.0, hint, theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
    }
}

fn draw_power(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "Power Configuration");
    let mut col = Column::new(inner).with_gap(8.0);

    // IO Banks
    let r = col.next(24.0);
    ui.label(r.x, r.y, "IO Bank Voltages (V)", theme::FONT_SIZE_NORMAL, theme::TEXT_PRIMARY);

    let banks = ["B13", "B33", "B34", "B35"];
    for bank in &banks {
        let r = col.next(24.0);
        ui.label(r.x, r.y + 4.0, bank, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
        ui.label(r.x + 60.0, r.y + 4.0, "0.000", theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
    }

    // VICOR cores
    let r = col.next(28.0);
    ui.label(r.x, r.y + 4.0, "VICOR Cores", theme::FONT_SIZE_NORMAL, theme::TEXT_PRIMARY);

    let widths = [60.0, 80.0, 80.0, 80.0];
    let hdr_r = col.next(24.0);
    ui.table_header(hdr_r, &["Core", "Voltage", "Delay", ""], &widths);

    for i in 1..=6 {
        let r = col.next(24.0);
        ui.table_row(r, &[&format!("{}", i), "0.000 V", "100 ms", ""], &widths, i % 2 == 0);
    }
}

fn draw_timing(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "Pin Configuration");
    let mut col = Column::new(inner).with_gap(8.0);

    let r = col.next(24.0);
    ui.label(r.x, r.y, "Pin type assignments from .tim file", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);

    let widths = [60.0, 80.0, 60.0, 60.0, 60.0, 60.0];
    let hdr_r = col.next(24.0);
    ui.table_header(hdr_r, &["Pin", "Type", "PType", "Rise", "Fall", "Period"], &widths);

    let r = col.next(24.0);
    ui.label(r.x + 12.0, r.y + 4.0, "Load a test plan to view pin configs", theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
}

fn draw_vectors(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "Vector Files");
    let mut col = Column::new(inner).with_gap(8.0);

    let r = col.next(24.0);
    ui.label(r.x, r.y, "Test vector file paths", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);

    let r = col.next(24.0);
    ui.label(r.x, r.y, ".seq path:", theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    let r = col.next(24.0);
    ui.label(r.x, r.y, ".hex / .fbc path:", theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
}

fn draw_thermal(ui: &mut Ui, body: Rect, _state: &mut AppState) {
    let inner = ui.card(body, "Thermal Configuration");
    let mut col = Column::new(inner).with_gap(8.0);

    let fields = [
        ("Setpoint", "Target temperature (C) — leave empty for ambient"),
        ("R25C", "Thermistor resistance at 25C (ohms, default 30000)"),
        ("Cool After", "Cool to ambient after test completes"),
    ];

    for (label, hint) in &fields {
        let r = col.next(28.0);
        ui.label(r.x, r.y + 4.0, label, theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
        ui.label(r.x + 120.0, r.y + 4.0, hint, theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
    }

    // ADC checks
    let r = col.next(28.0);
    ui.label(r.x, r.y + 4.0, "ADC Checks (post-power-on validation)", theme::FONT_SIZE_NORMAL, theme::TEXT_PRIMARY);

    let widths = [80.0, 100.0, 100.0];
    let hdr_r = col.next(24.0);
    ui.table_header(hdr_r, &["Channel", "Min (mV)", "Max (mV)"], &widths);

    let r = col.next(24.0);
    ui.label(r.x + 12.0, r.y + 4.0, "Load a test plan to view ADC checks", theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
}
