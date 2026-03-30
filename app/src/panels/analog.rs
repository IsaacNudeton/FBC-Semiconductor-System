/// Analog panel — 32-channel ADC monitor (XADC + MAX11131)
/// Wires real AnalogChannels data from board state.

use crate::ui::Ui;
use crate::layout::{Rect, Column};
use crate::state::AppState;
use crate::transport::HwCommand;
use crate::theme;

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    // Header
    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Analog Monitor", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    let refresh_r = header.right_align(100.0, 36.0);
    if ui.button(refresh_r, "Read All") {
        if let Some(board) = state.selected_board.clone() {
            state.send_command(HwCommand::ReadAnalog(board));
        }
    }

    // Tabs: XADC | External ADC | All
    let tab_r = col.next(36.0);
    let tab_labels = ["XADC (0-15)", "External (16-31)", "All"];
    let active_tab = state.tab_index("analog");
    if let Some(idx) = ui.tabs(tab_r, &tab_labels, active_tab) {
        state.set_tab_index("analog", idx);
    }
    let active_tab = state.tab_index("analog");

    // Get analog data from selected board
    let analog = state.selected_board_state().and_then(|b| b.analog.clone());

    // Channel grid — 4 columns x 8 rows = 32 channels
    let grid_area = col.remaining();
    let cells = grid_area.grid(4, 8, 8.0);

    let (start_ch, end_ch) = match active_tab {
        0 => (0usize, 16usize),
        1 => (16, 32),
        _ => (0, 32),
    };

    for i in start_ch..end_ch {
        let grid_idx = if active_tab == 2 { i } else { i - start_ch };
        if grid_idx >= cells.len() { break; }
        let cell = cells[grid_idx];

        // Channel card
        ui.draw.rounded_rect(cell, theme::BORDER_RADIUS, theme::BG_SECONDARY);
        ui.draw.border(cell, 1.0, theme::BORDER);

        let inner = cell.padded(6.0);
        let ch_label = format!("CH{}", i);
        ui.label(inner.x, inner.y, &ch_label, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

        // Try to find voltage from analog data
        let voltage = analog.as_ref().and_then(|a| {
            if i < 16 {
                a.xadc.iter().find(|r| r.channel == i as u8).map(|r| r.voltage_mv)
            } else {
                a.external.iter().find(|r| r.channel == i as u8).map(|r| r.voltage_mv)
            }
        });

        match voltage {
            Some(mv) => {
                let color = if mv > 0.0 && mv < 3600.0 { theme::SUCCESS } else { theme::WARNING };
                ui.label(inner.x, inner.y + 18.0, &format!("{:.1} mV", mv), theme::FONT_SIZE_NORMAL, color);
                // Progress bar (scale 0-3300mV)
                let bar_r = Rect::new(inner.x, inner.y + 40.0, inner.w, 4.0);
                let frac = (mv / 3300.0).clamp(0.0, 1.0);
                ui.progress(bar_r, frac, color);
            }
            None => {
                ui.label(inner.x, inner.y + 18.0, "--- mV", theme::FONT_SIZE_NORMAL, theme::TEXT_DISABLED);
                let bar_r = Rect::new(inner.x, inner.y + 40.0, inner.w, 4.0);
                ui.progress(bar_r, 0.0, theme::ACCENT);
            }
        }
    }
}
