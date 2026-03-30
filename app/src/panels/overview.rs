/// Overview panel — unified fleet summary, board list, connection controls.
/// Both FBC and Sonoma boards in a single table, no mode toggle.
/// Supports multi-select for orchestration (Ctrl+click or checkboxes).

use crate::ui::Ui;
use crate::layout::{Rect, Column, Row};
use crate::state::AppState;
use crate::transport::HwCommand;
use crate::theme;

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    // Header
    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Fleet Overview", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    // Connection controls row
    let conn_row = col.next(44.0);
    let mut row = Row::new(conn_row).with_gap(8.0);

    // Discover All button
    let discover_r = row.next(120.0);
    if ui.button(discover_r, "Discover All") {
        state.send_command(HwCommand::DiscoverAll {
            interface: state.interface.clone(),
            timeout_ms: 2000,
            start: state.sonoma_range_start.clone(),
            end: state.sonoma_range_end.clone(),
            user: state.sonoma_user.clone(),
            password: state.sonoma_password.clone(),
        });
    }

    // FBC-only discover
    let fbc_r = row.next(100.0);
    if ui.button_ghost(fbc_r, "FBC Only") {
        state.send_command(HwCommand::Discover {
            interface: state.interface.clone(),
            timeout_ms: 2000,
        });
    }

    // Sonoma-only scan
    let sonoma_r = row.next(110.0);
    if ui.button_ghost(sonoma_r, "Sonoma Only") {
        state.send_command(HwCommand::ScanSonoma {
            start: state.sonoma_range_start.clone(),
            end: state.sonoma_range_end.clone(),
            user: state.sonoma_user.clone(),
            password: state.sonoma_password.clone(),
        });
    }

    // Status
    let status_r = row.remaining();
    ui.label(status_r.x + 8.0, status_r.y + 12.0, &state.status_message, theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);

    // Target selector row
    let target_row = col.next(32.0);
    if let Some(new_target) = ui.target_selector(target_row, &state.command_target, "Target:") {
        state.command_target = new_target;
    }

    // Target summary + count
    let summary = col.next(24.0);
    let fbc_count = state.boards.iter().filter(|b| b.is_fbc()).count();
    let sonoma_count = state.boards.iter().filter(|b| b.is_sonoma()).count();
    let target_text = format!(
        "{} boards ({} FBC, {} Sonoma) | Target: {}",
        state.boards.len(), fbc_count, sonoma_count,
        state.target_label(),
    );
    ui.label(summary.x, summary.y + 4.0, &target_text, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    // Separator
    col.next(1.0);
    ui.separator(col.next(1.0));

    // Board list table
    let table_rect = col.remaining();

    // Table header — added checkbox column for multi-select
    let widths = [30.0, 70.0, 180.0, 100.0, 80.0, 80.0, 100.0];
    let header_r = Rect::new(table_rect.x, table_rect.y, table_rect.w, theme::ROW_HEIGHT);
    ui.table_header(header_r, &["", "Type", "Address", "FW Version", "State", "Temp", "Status"], &widths);

    let mut y = table_rect.y + theme::ROW_HEIGHT;

    // Collect board data to avoid borrow issues
    let board_data: Vec<_> = state.boards.iter().enumerate().map(|(i, board)| {
        let id = board.id.clone();
        let is_selected = state.selected_board.as_ref() == Some(&id);
        let is_multi = state.multi_select.contains(&i);
        let type_label = board.type_label().to_string();
        let is_fbc = board.is_fbc();
        let label = board.label.clone();
        let fw = board.fw_version.clone();
        let alive = board.alive;
        let state_str = board.status.as_ref()
            .map(|s| format!("{}", s.state))
            .unwrap_or_else(|| if alive { "Ready".into() } else { "Offline".into() });
        let state_color = board.status.as_ref()
            .map(|s| match s.state {
                fbc_host::types::ControllerState::Running => theme::SUCCESS,
                fbc_host::types::ControllerState::Error => theme::ERROR,
                fbc_host::types::ControllerState::Done => theme::ACCENT,
                fbc_host::types::ControllerState::Idle => theme::IDLE,
            })
            .unwrap_or(if alive { theme::TEXT_SECONDARY } else { theme::TEXT_DISABLED });
        let temp_str = board.status.as_ref()
            .map(|s| format!("{:.1}C", s.temp_c))
            .unwrap_or_default();
        (i, id, is_selected, is_multi, type_label, is_fbc, label, fw, alive, state_str, state_color, temp_str)
    }).collect();

    let mut selection_change = None;
    let mut multi_toggle = None;

    for (i, id, is_selected, is_multi, type_label, is_fbc, label, fw, alive, state_str, state_color, temp_str) in &board_data {
        let row_r = Rect::new(table_rect.x, y, table_rect.w, theme::ROW_HEIGHT);

        // Highlight selected
        if *is_selected {
            ui.draw.rounded_rect(row_r, 2.0, theme::ACCENT_DIM);
        }

        // Multi-select checkbox
        let cb_r = Rect::new(row_r.x + 4.0, row_r.y + 2.0, 26.0, row_r.h - 4.0);
        if let Some(_new_val) = ui.checkbox(cb_r, "", *is_multi) {
            multi_toggle = Some(*i);
        }

        // Type badge
        let type_color = if *is_fbc { theme::ACCENT } else { theme::SUCCESS };
        ui.badge(row_r.x + 8.0 + widths[0], row_r.y + 8.0, type_label, type_color);

        // Address
        let addr_x = row_r.x + 8.0 + widths[0] + widths[1];
        ui.label(addr_x, row_r.y + 10.0, label, theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);

        // FW version
        let fw_x = addr_x + widths[2];
        ui.label(fw_x, row_r.y + 10.0, fw, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

        // State
        let state_x = fw_x + widths[3];
        ui.label(state_x, row_r.y + 10.0, state_str, theme::FONT_SIZE_SMALL, *state_color);

        // Temp
        let temp_x = state_x + widths[4];
        ui.label(temp_x, row_r.y + 10.0, temp_str, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

        // Online/Offline
        let status_x = temp_x + widths[5];
        let (status_text, status_color) = if *alive {
            ("Online", theme::SUCCESS)
        } else {
            ("Offline", theme::ERROR)
        };
        ui.label(status_x, row_r.y + 10.0, status_text, theme::FONT_SIZE_SMALL, status_color);

        // Click row to select (single-click = select, checkbox = multi-select)
        let row_click_area = Rect::new(row_r.x + widths[0], row_r.y, row_r.w - widths[0], row_r.h);
        if ui.input.clicked_in(row_click_area.x, row_click_area.y, row_click_area.w, row_click_area.h) {
            selection_change = Some(id.clone());
        }

        y += theme::ROW_HEIGHT;
    }

    // Apply deferred mutations
    if let Some(id) = selection_change {
        state.selected_board = Some(id);
    }
    if let Some(idx) = multi_toggle {
        state.toggle_multi_select(idx);
    }

    if state.boards.is_empty() {
        ui.label(
            table_rect.x + 12.0, y + 12.0,
            "No boards discovered. Click Discover All to scan FBC + Sonoma.",
            theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY,
        );
    }
}
