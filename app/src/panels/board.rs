/// Board detail panel — identity, FPGA voltages, status, controls
/// Unified for both FBC and Sonoma boards.

use crate::ui::Ui;
use crate::layout::{Rect, Column, Row};
use crate::state::AppState;
use crate::transport::HwCommand;
use crate::theme;

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Board Detail", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    // Control buttons
    let btn_row = col.next(40.0);
    let mut row = Row::new(btn_row).with_gap(8.0);

    if ui.button(row.next(80.0), "Refresh") {
        if let Some(board) = state.selected_board.clone() {
            state.send_command(HwCommand::GetStatus(board));
        }
    }
    if ui.button(row.next(70.0), "Start") {
        if let Some(board) = state.selected_board.clone() {
            state.send_command(HwCommand::Start(board));
        }
    }
    if ui.button_ghost(row.next(70.0), "Stop") {
        if let Some(board) = state.selected_board.clone() {
            state.send_command(HwCommand::Stop(board));
        }
    }
    if ui.button_ghost(row.next(70.0), "Reset") {
        if let Some(board) = state.selected_board.clone() {
            state.send_command(HwCommand::Reset(board));
        }
    }
    if ui.button(row.next(70.0), "Ping") {
        if let Some(board) = state.selected_board.clone() {
            state.send_command(HwCommand::Ping(board));
        }
    }

    let selected = state.selected_board_state().cloned();
    match selected {
        Some(board) => {
            // Type badge
            let type_row = col.next(28.0);
            let type_color = if board.is_fbc() { theme::ACCENT } else { theme::SUCCESS };
            ui.badge(type_row.x, type_row.y, board.type_label(), type_color);
            let alive_color = if board.alive { theme::SUCCESS } else { theme::ERROR };
            ui.badge(type_row.x + 80.0, type_row.y, if board.alive { "Online" } else { "Offline" }, alive_color);

            // Identity card
            let info_area = col.next(200.0);
            let inner = ui.card(info_area, "Board Identity");
            let mut icol = Column::new(inner).with_gap(6.0);

            let mut fields: Vec<(&str, String)> = vec![
                ("Address", board.label.clone()),
                ("System", format!("{}", board.system_type)),
                ("FW Version", board.fw_version.clone()),
            ];

            // FBC-specific fields from BoardInfo
            if let Some(info) = &board.fbc_info {
                fields.push(("Serial", format!("{:08X}", info.serial)));
                fields.push(("HW Revision", format!("{}", info.hw_revision)));
                fields.push(("BIM Type", format!("{}", info.bim_type)));
                fields.push(("BIM Present", format!("{}", info.has_bim)));
                fields.push(("BIM Programmed", format!("{}", info.bim_programmed)));
            }

            for (label, value) in &fields {
                let r = icol.next(22.0);
                ui.label(r.x, r.y + 2.0, label, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
                ui.label(r.x + 140.0, r.y + 2.0, value, theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);
            }

            // Status card
            if let Some(status) = &board.status {
                let status_area = col.next(160.0);
                let sinner = ui.card(status_area, "Live Status");
                let mut scol = Column::new(sinner).with_gap(6.0);

                let (state_label, state_color) = match status.state {
                    fbc_host::types::ControllerState::Idle => ("IDLE", theme::IDLE),
                    fbc_host::types::ControllerState::Running => ("RUNNING", theme::SUCCESS),
                    fbc_host::types::ControllerState::Done => ("DONE", theme::ACCENT),
                    fbc_host::types::ControllerState::Error => ("ERROR", theme::ERROR),
                };

                let r = scol.next(24.0);
                ui.badge(r.x, r.y, state_label, state_color);

                let sfields = [
                    ("Cycles", format!("{}", status.cycles)),
                    ("Errors", format!("{}", status.errors)),
                    ("Temperature", format!("{:.1} C", status.temp_c)),
                    ("VCCINT", format!("{} mV", status.fpga_vccint)),
                    ("VCCAUX", format!("{} mV", status.fpga_vccaux)),
                ];

                for (label, value) in &sfields {
                    let r = scol.next(20.0);
                    ui.label(r.x, r.y, label, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
                    ui.label(r.x + 120.0, r.y, value, theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);
                }
            }

            // Sonoma-specific status
            if let Some(ss) = &board.sonoma_status {
                let ss_area = col.next(120.0);
                let ss_inner = ui.card(ss_area, "Sonoma Status");
                let mut ss_col = Column::new(ss_inner).with_gap(6.0);

                let r = ss_col.next(20.0);
                ui.label(r.x, r.y, "XADC Channels", theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
                ui.label(r.x + 140.0, r.y, &format!("{}", ss.xadc.len()), theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);

                let r = ss_col.next(20.0);
                ui.label(r.x, r.y, "ADC32 Channels", theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
                ui.label(r.x + 140.0, r.y, &format!("{}", ss.adc32.len()), theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);
            }
        }
        None => {
            ui.label(content.x, content.y + 60.0, "No board selected", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
        }
    }
}
