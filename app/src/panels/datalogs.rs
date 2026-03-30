/// Datalogs panel — binary datalog interpretation, LOT summaries, export.
/// Placeholder for now — will decode binary datalogs client-side,
/// correlate across boards, detect anomalies, and export professional reports.

use crate::ui::Ui;
use crate::layout::{Rect, Column};
use crate::state::AppState;
use crate::theme;

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    // Header
    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Datalogs", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);
    ui.label(header.x, header.y + 34.0,
        "Binary datalog decode, LOT summaries, anomaly detection, export",
        theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    // LOT summary section
    let lot_area = col.next(200.0);
    let lot_inner = ui.card(lot_area, "LOT History");

    if state.boards.is_empty() {
        ui.label(lot_inner.x, lot_inner.y + 8.0,
            "No boards discovered. Datalogs will appear here after burn-in runs complete.",
            theme::FONT_SIZE_NORMAL, theme::TEXT_DISABLED);
    } else {
        // Table header
        let hdr = Rect::new(lot_inner.x, lot_inner.y, lot_inner.w, 28.0);
        let widths = [100.0, 120.0, 120.0, 80.0, 100.0, 80.0];
        ui.table_header(hdr, &["LOT #", "Customer", "Device", "Boards", "Status", "Pass %"], &widths);

        // Placeholder row
        let row = Rect::new(lot_inner.x, lot_inner.y + 30.0, lot_inner.w, 28.0);
        ui.table_row(row, &["(pending)", "(pending)", "(pending)", "-", "No data", "-"], &widths, true);

        ui.label(lot_inner.x, lot_inner.y + 70.0,
            "Datalogs will auto-collect when boards complete burn-in runs.",
            theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
    }

    // Export section
    let export_area = col.next(120.0);
    let export_inner = ui.card(export_area, "Export");

    ui.label(export_inner.x, export_inner.y + 4.0,
        "Export formats: PDF (customer-facing), CSV (data analysis), JSON (LRM v2)",
        theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    let btn_y = export_inner.y + 28.0;
    let _pdf = ui.button(Rect::new(export_inner.x, btn_y, 100.0, 30.0), "Export PDF");
    let _csv = ui.button(Rect::new(export_inner.x + 110.0, btn_y, 100.0, 30.0), "Export CSV");
    let _json = ui.button(Rect::new(export_inner.x + 220.0, btn_y, 100.0, 30.0), "Export JSON");

    // Anomaly detection section
    let anomaly_area = col.next(140.0);
    let anomaly_inner = ui.card(anomaly_area, "Anomaly Detection");

    ui.label(anomaly_inner.x, anomaly_inner.y + 4.0,
        "Automatic detection: temperature outliers, voltage drift, correlated error spikes",
        theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
    ui.label(anomaly_inner.x, anomaly_inner.y + 24.0,
        "No anomalies detected (no active runs)",
        theme::FONT_SIZE_NORMAL, theme::TEXT_DISABLED);

    // Status
    let board_count = state.boards.len();
    let fbc = state.boards.iter().filter(|b| b.is_fbc()).count();
    let sonoma = state.boards.iter().filter(|b| b.is_sonoma()).count();
    let info = col.next(24.0);
    ui.label(info.x, info.y,
        &format!("{} boards ({} FBC, {} Sonoma) | Datalogs auto-sync to LRM v2 when connected", board_count, fbc, sonoma),
        theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
}
