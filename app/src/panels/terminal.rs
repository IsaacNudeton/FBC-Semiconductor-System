/// Terminal panel — SSH command interface for Sonoma boards, direct commands for FBC boards.
/// Quick command buttons dispatch HwCommand variants based on board type.

use crate::ui::Ui;
use crate::layout::{Rect, Column, Row};
use crate::state::AppState;
use crate::transport::{BoardId, HwCommand};
use crate::theme;

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    // Header
    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Terminal", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    // Connection info
    let info_r = col.next(32.0);
    let board = state.selected_board.clone();
    let is_sonoma = state.selected_board_state().map(|b| b.is_sonoma()).unwrap_or(false);

    match &board {
        Some(BoardId::Ip(ip)) => {
            ui.label(info_r.x, info_r.y + 4.0, "Board:", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
            ui.badge(info_r.x + 60.0, info_r.y + 4.0, ip, theme::SUCCESS);
            ui.badge(info_r.x + 220.0, info_r.y + 4.0, "Sonoma", theme::ACCENT);
        }
        Some(BoardId::Mac(mac)) => {
            let mac_str = format!(
                "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]
            );
            ui.label(info_r.x, info_r.y + 4.0, "Board:", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
            ui.badge(info_r.x + 60.0, info_r.y + 4.0, &mac_str, theme::SUCCESS);
            ui.badge(info_r.x + 220.0, info_r.y + 4.0, "FBC", theme::WARNING);
        }
        None => {
            ui.label(info_r.x, info_r.y + 4.0, "No board selected", theme::FONT_SIZE_NORMAL, theme::TEXT_DISABLED);
        }
    }

    // Quick command buttons
    let btn_row_r = col.next(40.0);

    if is_sonoma {
        // Sonoma quick commands
        if let Some(BoardId::Ip(ip)) = &board {
            let mut row = Row::new(btn_row_r).with_gap(8.0);

            let r = row.next(80.0);
            if ui.button(r, "Init") {
                state.send_command(HwCommand::SonomaInit(ip.clone()));
            }

            let r = row.next(80.0);
            if ui.button(r, "Status") {
                state.send_command(HwCommand::SonomaGetStatus(ip.clone()));
            }

            let r = row.next(80.0);
            if ui.button(r, "XADC") {
                state.send_command(HwCommand::SonomaReadXadc(ip.clone()));
            }

            let r = row.next(80.0);
            if ui.button(r, "ADC32") {
                state.send_command(HwCommand::SonomaReadAdc32(ip.clone()));
            }

            let r = row.next(100.0);
            if ui.button(r, "FW Version") {
                state.send_command(HwCommand::GetFirmwareInfo(BoardId::Ip(ip.clone())));
            }

            let r = row.next(100.0);
            if ui.button(r, "Toggle MIO") {
                state.send_command(HwCommand::SonomaToggleMio(ip.clone(), 36, 1));
            }
        }
    } else {
        // FBC quick commands
        if let Some(BoardId::Mac(mac)) = &board {
            let mac = *mac;
            let mut row = Row::new(btn_row_r).with_gap(8.0);

            let r = row.next(80.0);
            if ui.button(r, "Status") {
                state.send_command(HwCommand::GetStatus(BoardId::Mac(mac)));
            }

            let r = row.next(80.0);
            if ui.button(r, "Analog") {
                state.send_command(HwCommand::ReadAnalog(BoardId::Mac(mac)));
            }

            let r = row.next(80.0);
            if ui.button(r, "Ping") {
                state.send_command(HwCommand::Ping(BoardId::Mac(mac)));
            }

            let r = row.next(80.0);
            if ui.button(r, "Errors") {
                state.send_command(HwCommand::GetErrorLog(BoardId::Mac(mac), 0, 10));
            }

            let r = row.next(100.0);
            if ui.button(r, "FW Info") {
                state.send_command(HwCommand::GetFirmwareInfo(BoardId::Mac(mac)));
            }
        }
    }

    // No board selected — show disabled placeholder buttons
    if board.is_none() {
        let mut row = Row::new(btn_row_r).with_gap(8.0);
        for label in &["Status", "Analog", "Ping", "Errors", "FW Info"] {
            let r = row.next(80.0);
            ui.button_ghost(r, label);
        }
    }

    // Command history / output area (dark card)
    let output_area = col.remaining();
    let output_card = output_area.take_top(output_area.h - 44.0);
    ui.draw.rounded_rect(output_card, theme::BORDER_RADIUS, theme::BG_PRIMARY);
    ui.draw.border(output_card, 1.0, theme::BORDER);

    let output_inner = output_card.padded(12.0);

    // Show status_message as the most recent result
    if !state.status_message.is_empty() {
        ui.label(
            output_inner.x,
            output_inner.y,
            "> ",
            theme::FONT_SIZE_NORMAL,
            theme::SUCCESS,
        );
        ui.label(
            output_inner.x + 16.0,
            output_inner.y,
            &state.status_message,
            theme::FONT_SIZE_NORMAL,
            theme::TEXT_PRIMARY,
        );
    } else {
        ui.label(
            output_inner.x,
            output_inner.y,
            "No output yet. Select a board and run a command.",
            theme::FONT_SIZE_NORMAL,
            theme::TEXT_DISABLED,
        );
    }

    // Execute row — hint text
    let exec_r = Rect::new(
        output_area.x,
        output_card.bottom() + 8.0,
        output_area.w,
        32.0,
    );
    ui.label(
        exec_r.x,
        exec_r.y + 6.0,
        "Use CLI for direct commands",
        theme::FONT_SIZE_SMALL,
        theme::TEXT_SECONDARY,
    );
}
