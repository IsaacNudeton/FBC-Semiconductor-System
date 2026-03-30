/// EEPROM panel — hex viewer + editor with real data from board

use crate::ui::Ui;
use crate::layout::{Rect, Column, Row};
use crate::state::AppState;
use crate::transport::HwCommand;
use crate::theme;

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "EEPROM", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    let btn_row = col.next(40.0);
    let mut row = Row::new(btn_row).with_gap(8.0);

    if ui.button(row.next(100.0), "Read All") {
        if let Some(board) = state.selected_board.clone() {
            state.send_command(HwCommand::ReadEeprom(board, 0, 255));
        }
    }

    if ui.button_ghost(row.next(100.0), "Write") {
        // Write back current eeprom_data if edited
        if let Some(board_id) = state.selected_board.clone() {
            if let Some(board) = state.selected_board_state() {
                if let Some(eeprom) = &board.eeprom_data {
                    state.send_command(HwCommand::WriteEeprom(
                        board_id,
                        eeprom.offset,
                        eeprom.data.clone(),
                    ));
                }
            }
        }
    }

    // Hex view area
    let hex_area = col.remaining();
    let inner = ui.card(hex_area, "Hex View");

    // Column headers: Offset | 00 01 02 ... 0F | ASCII
    let hdr_r = Rect::new(inner.x, inner.y, inner.w, 20.0);
    let mut header_text = String::from("Offset   ");
    for i in 0..16 {
        header_text.push_str(&format!("{:02X} ", i));
    }
    header_text.push_str("  ASCII");
    ui.label(hdr_r.x, hdr_r.y, &header_text, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    // Get EEPROM data from board
    let eeprom_data = state.selected_board_state().and_then(|b| b.eeprom_data.as_ref());

    let mut y = inner.y + 24.0;
    for row_idx in 0..16 {
        let offset = row_idx * 16;
        let mut line = format!("{:04X}     ", offset);
        let mut ascii = String::new();

        for col_idx in 0..16 {
            let byte_idx = offset + col_idx;
            match eeprom_data {
                Some(data) if byte_idx < data.data.len() => {
                    let b = data.data[byte_idx];
                    line.push_str(&format!("{:02X} ", b));
                    ascii.push(if b >= 0x20 && b < 0x7F { b as char } else { '.' });
                }
                _ => {
                    line.push_str("-- ");
                    ascii.push('.');
                }
            }
        }
        line.push_str("  ");
        line.push_str(&ascii);

        let color = if eeprom_data.is_some() { theme::TEXT_PRIMARY } else { theme::TEXT_DISABLED };
        ui.label(inner.x, y, &line, theme::FONT_SIZE_SMALL, color);
        y += 18.0;
    }
}
