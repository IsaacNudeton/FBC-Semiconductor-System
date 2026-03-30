/// Power panel — VICOR 6-core control + PMBus rails + Emergency Stop
/// Works for both FBC (direct register) and Sonoma (SSH ELFs).
/// Supports orchestration: commands go to current target (one/many/all).

use crate::ui::Ui;
use crate::layout::{Rect, Column};
use crate::state::AppState;
use crate::transport::HwCommand;
use crate::theme;

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    // Header with emergency stop
    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Power Control", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    let estop_r = header.right_align(160.0, 40.0);
    if ui.button_colored(estop_r, "EMERGENCY STOP", theme::ERROR, theme::EMERGENCY) {
        // E-stop goes to ALL targets
        state.send_to_targets(|id| HwCommand::EmergencyStop(id));
    }

    let refresh_r = Rect::new(estop_r.x - 100.0, header.y + 4.0, 88.0, 36.0);
    if ui.button(refresh_r, "Refresh") {
        state.send_to_targets(|id| HwCommand::GetVicorStatus(id));
    }

    // Target selector
    let target_row = col.next(28.0);
    let target_label = format!("Target: {}", state.target_label());
    if let Some(new_target) = ui.target_selector(target_row, &state.command_target, &target_label) {
        state.command_target = new_target;
    }

    // VICOR Cores — 3x2 grid (shows data from selected board)
    let grid_area = col.next(280.0);
    let cells = grid_area.grid(3, 2, 12.0);

    let cores = state.selected_board_state()
        .and_then(|b| b.vicor.as_ref())
        .map(|v| v.cores);

    for i in 0..6 {
        if i >= cells.len() { break; }
        let cell = cells[i];
        let core_area = ui.card(cell, &format!("Core {}", i + 1));

        if let Some(cores) = &cores {
            let core = &cores[i];
            let mut inner = Column::new(core_area).with_gap(6.0);

            // Enable toggle — sends to ALL targets
            let toggle_r = inner.next(28.0);
            if let Some(new_val) = ui.toggle(toggle_r, "Enable", core.enabled) {
                let mut mask: u8 = 0;
                for (j, c) in cores.iter().enumerate() {
                    if (j == i && new_val) || (j != i && c.enabled) {
                        mask |= 1 << j;
                    }
                }
                state.send_to_targets(|id| HwCommand::SetVicorEnable(id, mask));
            }

            // Voltage
            let volt_r = inner.next(28.0);
            let v_color = if core.enabled { theme::SUCCESS } else { theme::TEXT_SECONDARY };
            ui.label(volt_r.x, volt_r.y + 4.0, &format!("{} mV", core.voltage_mv), theme::FONT_SIZE_LARGE, v_color);

            // Current
            let curr_r = inner.next(20.0);
            ui.label(curr_r.x, curr_r.y, &format!("{} mA", core.current_ma), theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);

            // Voltage slider — sends to ALL targets
            let slider_r = inner.next(36.0);
            if let Some(new_v) = ui.slider(slider_r, "", core.voltage_mv as f32, 0.0, 3300.0) {
                let core_idx = i as u8;
                let mv = new_v as u16;
                state.send_to_targets(move |id| HwCommand::SetVicorVoltage(id, core_idx, mv));
            }
        } else {
            ui.label(core_area.x, core_area.y + 8.0, "No data", theme::FONT_SIZE_NORMAL, theme::TEXT_DISABLED);
        }
    }

    // PMBus section
    let pmbus_header = col.next(32.0);
    ui.label(pmbus_header.x, pmbus_header.y + 4.0, "PMBus Rails", theme::FONT_SIZE_LARGE, theme::TEXT_PRIMARY);

    let table_rect = col.remaining();
    let widths = [80.0, 80.0, 100.0, 100.0, 100.0];
    let hdr_r = Rect::new(table_rect.x, table_rect.y, table_rect.w, theme::ROW_HEIGHT);
    ui.table_header(hdr_r, &["Address", "Enabled", "Voltage", "Current", ""], &widths);

    let pmbus = state.selected_board_state().and_then(|b| b.pmbus.as_ref());
    if let Some(pmbus) = pmbus {
        let mut y = table_rect.y + theme::ROW_HEIGHT;
        for (i, rail) in pmbus.rails.iter().enumerate() {
            let row_r = Rect::new(table_rect.x, y, table_rect.w, theme::ROW_HEIGHT);
            let enabled_str = if rail.enabled { "ON" } else { "OFF" };
            ui.table_row(
                row_r,
                &[
                    &format!("0x{:02X}", rail.address),
                    enabled_str,
                    &format!("{} mV", rail.voltage_mv),
                    &format!("{} mA", rail.current_ma),
                    "",
                ],
                &widths,
                i % 2 == 0,
            );
            y += theme::ROW_HEIGHT;
        }
    } else {
        ui.label(
            table_rect.x + 12.0,
            table_rect.y + theme::ROW_HEIGHT + 12.0,
            "Select a board and click Refresh",
            theme::FONT_SIZE_NORMAL,
            theme::TEXT_SECONDARY,
        );
    }
}
