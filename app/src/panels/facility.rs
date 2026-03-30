/// Facility panel — fleet management, multi-board overview, slot map, thermal overview.
/// Provides batch operations (Run All, Stop All, Poll All) across discovered boards.

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
    ui.label(header.x, header.y + 8.0, "Fleet Management", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    // Summary row
    let summary_row = col.next(44.0);
    let mut row = Row::new(summary_row).with_gap(8.0);

    let fbc_count = state.boards.iter().filter(|b| b.is_fbc()).count();
    let sonoma_count = state.boards.iter().filter(|b| b.is_sonoma()).count();
    let total = state.boards.len();
    let summary_text = format!("{} boards ({} FBC, {} Sonoma)", total, fbc_count, sonoma_count);

    let label_r = row.next(280.0);
    ui.label(label_r.x, label_r.y + 12.0, &summary_text, theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);

    // Run All → sends to current target
    let run_r = row.next(90.0);
    if ui.button_colored(run_r, "Run All", theme::BG_TERTIARY, theme::SUCCESS) {
        state.send_to_targets(|id| HwCommand::Start(id));
    }

    // Stop All → sends to current target
    let stop_r = row.next(90.0);
    if ui.button_colored(stop_r, "Stop All", theme::BG_TERTIARY, theme::EMERGENCY) {
        state.send_to_targets(|id| HwCommand::EmergencyStop(id));
    }

    // Poll All → sends to current target
    let poll_r = row.next(90.0);
    if ui.button(poll_r, "Poll All") {
        state.send_to_targets(|id| HwCommand::GetStatus(id));
    }

    // Target selector
    let target_row = col.next(28.0);
    if let Some(new_target) = ui.target_selector(target_row, &state.command_target, "Target:") {
        state.command_target = new_target;
    }

    // Tabs
    let tab_r = col.next(36.0);
    let tab_labels = ["Board Grid", "Slot Map", "Thermal Overview", "Network"];
    let active_tab = state.tab_index("facility");
    if let Some(idx) = ui.tabs(tab_r, &tab_labels, active_tab) {
        state.set_tab_index("facility", idx);
    }
    let active_tab = state.tab_index("facility");

    let body = col.remaining();

    match active_tab {
        0 => draw_board_grid(ui, body, state),
        1 => draw_slot_map(ui, body, state),
        2 => draw_thermal_overview(ui, body, state),
        3 => draw_network(ui, body, state),
        _ => {}
    }
}

/// Board Grid — small status cards in a grid, color-coded by state.
fn draw_board_grid(ui: &mut Ui, rect: Rect, state: &AppState) {
    if state.boards.is_empty() {
        ui.label(
            rect.x + 12.0, rect.y + 12.0,
            "No boards discovered. Use Overview to discover boards.",
            theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY,
        );
        return;
    }

    // Determine grid dimensions: up to 6 columns, rows as needed
    let cols = 6usize.min(state.boards.len());
    let rows = (state.boards.len() + cols - 1) / cols;
    let cells = rect.grid(cols, rows, 8.0);

    for (i, board) in state.boards.iter().enumerate() {
        if i >= cells.len() { break; }
        let cell = cells[i];

        // Card background — color by state
        let bg = board_state_bg(board);
        ui.draw.rounded_rect(cell, theme::BORDER_RADIUS, bg);
        ui.draw.border(cell, 1.0, theme::BORDER);

        let inner = cell.padded(8.0);

        // Type badge (top-left)
        let type_color = if board.is_fbc() { theme::ACCENT } else { theme::SUCCESS };
        ui.badge(inner.x, inner.y, board.type_label(), type_color);

        // Address (truncated to fit card)
        let addr = truncate_addr(&board.label, 18);
        ui.label(inner.x, inner.y + 24.0, &addr, theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);

        // State badge
        let (state_str, state_color) = board_state_label(board);
        ui.badge(inner.x, inner.y + 44.0, &state_str, state_color);

        // Temperature (bottom-right area)
        let temp_str = board.status.as_ref()
            .map(|s| format!("{:.1}C", s.temp_c))
            .unwrap_or_else(|| "---".into());
        let temp_color = board.status.as_ref()
            .map(|s| temp_color(s.temp_c))
            .unwrap_or(theme::TEXT_DISABLED);
        ui.label(inner.x + inner.w - 50.0, inner.y + 44.0, &temp_str, theme::FONT_SIZE_SMALL, temp_color);
    }
}

/// Slot Map — rack layout table. Sonoma slots derived from IP, FBC sequential.
fn draw_slot_map(ui: &mut Ui, rect: Rect, state: &AppState) {
    let widths = [60.0, 60.0, 80.0, 180.0, 70.0, 80.0, 70.0];
    let header_r = Rect::new(rect.x, rect.y, rect.w, theme::ROW_HEIGHT);
    ui.table_header(header_r, &["Slot", "Shelf", "Position", "Address", "Type", "State", "Temp"], &widths);

    if state.boards.is_empty() {
        ui.label(
            rect.x + 12.0, rect.y + theme::ROW_HEIGHT + 12.0,
            "No boards discovered.",
            theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY,
        );
        return;
    }

    let mut y = rect.y + theme::ROW_HEIGHT;

    for (i, board) in state.boards.iter().enumerate() {
        let row_r = Rect::new(rect.x, y, rect.w, theme::ROW_HEIGHT);

        let (slot, shelf, position) = if board.is_sonoma() {
            sonoma_slot_info(&board.label)
        } else {
            // FBC: sequential numbering
            let slot_num = i + 1;
            (format!("{}", slot_num), "1".into(), "---".into())
        };

        let (state_str, _) = board_state_label(board);
        let temp_str = board.status.as_ref()
            .map(|s| format!("{:.1}C", s.temp_c))
            .unwrap_or_else(|| "---".into());

        ui.table_row(
            row_r,
            &[&slot, &shelf, &position, &board.label, board.type_label(), &state_str, &temp_str],
            &widths,
            i % 2 == 0,
        );

        y += theme::ROW_HEIGHT;
    }
}

/// Thermal Overview — per-board thermal display with color-coded temps.
fn draw_thermal_overview(ui: &mut Ui, rect: Rect, state: &AppState) {
    let widths = [180.0, 90.0, 90.0, 90.0, 100.0];
    let header_r = Rect::new(rect.x, rect.y, rect.w, theme::ROW_HEIGHT);
    ui.table_header(header_r, &["Address", "Die Temp", "Case Temp", "Target", "Status"], &widths);

    if state.boards.is_empty() {
        ui.label(
            rect.x + 12.0, rect.y + theme::ROW_HEIGHT + 12.0,
            "No boards discovered.",
            theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY,
        );
        return;
    }

    let mut y = rect.y + theme::ROW_HEIGHT;

    for (i, board) in state.boards.iter().enumerate() {
        let row_r = Rect::new(rect.x, y, rect.w, theme::ROW_HEIGHT);

        let die_temp = board.status.as_ref()
            .map(|s| format!("{:.1}C", s.temp_c))
            .unwrap_or_else(|| "---".into());

        // Case temp from analog data (MAX11131 ch22 = THERM_CASE)
        let case_temp = board.analog.as_ref()
            .and_then(|a| a.external.iter().find(|r| r.channel == 22))
            .map(|r| format!("{:.1}C", ntc_estimate(r.voltage_mv)))
            .unwrap_or_else(|| "---".into());

        // Target from status (not directly available, show placeholder)
        let target = "125.0C".to_string();

        // Thermal status: color-coded
        let (thermal_status, thermal_color) = board.status.as_ref()
            .map(|s| {
                if s.temp_c > 120.0 {
                    ("HOT", theme::ERROR)
                } else if s.temp_c > 80.0 {
                    ("WARM", theme::WARNING)
                } else {
                    ("OK", theme::SUCCESS)
                }
            })
            .unwrap_or(("N/A", theme::TEXT_DISABLED));

        // Draw row with color coding on the temp columns
        ui.table_row(
            row_r,
            &[&board.label, &die_temp, &case_temp, &target, thermal_status],
            &widths,
            i % 2 == 0,
        );

        // Overlay color on die temp cell
        if let Some(status) = &board.status {
            let die_color = temp_color(status.temp_c);
            let die_x = row_r.x + widths[0];
            ui.label(die_x + 4.0, row_r.y + 10.0, &die_temp, theme::FONT_SIZE_SMALL, die_color);
        }

        // Overlay color on status cell
        let status_x = row_r.x + widths[0] + widths[1] + widths[2] + widths[3];
        ui.label(status_x + 4.0, row_r.y + 10.0, thermal_status, theme::FONT_SIZE_SMALL, thermal_color);

        y += theme::ROW_HEIGHT;
    }
}

/// Network — Cisco switch port map, MAC→board cross-reference, VLAN config.
fn draw_network(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(4.0);
    let mut col = Column::new(content).with_gap(8.0);

    // Connection controls row
    let conn_row = col.next(36.0);
    let mut row = Row::new(conn_row).with_gap(8.0);

    let port_label_r = row.next(60.0);
    ui.label(port_label_r.x, port_label_r.y + 8.0, "Port:", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);

    let port_r = row.next(80.0);
    let com_port = state.switch.com_port.clone();
    let cursor_val = *state.cursors.entry(0xDEAD_0001).or_insert(0);
    let mut cursor_mut = cursor_val;
    if let Some(new_val) = ui.text_input(port_r, 0xDEAD_0001, &com_port, &mut cursor_mut) {
        state.switch.com_port = new_val;
    }
    state.cursors.insert(0xDEAD_0001, cursor_mut);

    let btn_r = row.next(100.0);
    if state.switch.connected {
        if ui.button_colored(btn_r, "Disconnect", theme::BG_TERTIARY, theme::ERROR) {
            state.send_command(HwCommand::SwitchDisconnect);
        }
    } else {
        let port = state.switch.com_port.clone();
        if ui.button(btn_r, "Connect") {
            state.send_command(HwCommand::SwitchConnect { com_port: port });
        }
    }

    // Status indicator
    let status_r = row.next(200.0);
    if state.switch.connected {
        let status_text = format!("{} (connected)", state.switch.hostname);
        ui.label(status_r.x, status_r.y + 8.0, &status_text, theme::FONT_SIZE_NORMAL, theme::SUCCESS);
    } else {
        ui.label(status_r.x, status_r.y + 8.0, "Disconnected", theme::FONT_SIZE_NORMAL, theme::TEXT_DISABLED);
    }

    // Poll + action buttons (only when connected)
    if state.switch.connected {
        let poll_r = row.next(90.0);
        if ui.button(poll_r, "Poll Ports") {
            state.send_command(HwCommand::SwitchPollPorts);
        }
    }

    // Error display
    if let Some(err) = &state.switch.last_error {
        let err_r = col.next(24.0);
        let err_text = err.clone();
        ui.label(err_r.x, err_r.y + 4.0, &err_text, theme::FONT_SIZE_SMALL, theme::ERROR);
    }

    // Port table
    let body = col.remaining();
    let widths = [70.0, 80.0, 90.0, 60.0, 70.0, 70.0, 140.0, 160.0];
    let header_r = Rect::new(body.x, body.y, body.w, theme::ROW_HEIGHT);
    ui.table_header(header_r, &["Port", "Status", "VLAN", "Speed", "Duplex", "Link", "MAC", "Board"], &widths);

    if state.switch.ports.is_empty() {
        let msg = if state.switch.connected {
            "Click 'Poll Ports' to read switch port status."
        } else {
            "Connect to switch to view port map."
        };
        ui.label(
            body.x + 12.0, body.y + theme::ROW_HEIGHT + 12.0,
            msg, theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY,
        );
        return;
    }

    let mut y = body.y + theme::ROW_HEIGHT;
    for (i, port) in state.switch.ports.iter().enumerate() {
        if y + theme::ROW_HEIGHT > body.y + body.h {
            break;
        }
        let row_r = Rect::new(body.x, y, body.w, theme::ROW_HEIGHT);

        // Link status color
        let link_str = if port.status == "connected" { "UP" } else { "DOWN" };

        // Board cross-reference
        let board_label = port.board_id.as_ref()
            .map(|id| crate::state::AppState::board_label(id))
            .unwrap_or_default();

        ui.table_row(
            row_r,
            &[&port.port, &port.status, &port.vlan, &port.speed, &port.duplex, link_str, &port.mac_address, &board_label],
            &widths,
            i % 2 == 0,
        );

        // Color overlay: link status
        let link_x = row_r.x + widths[0] + widths[1] + widths[2] + widths[3] + widths[4];
        let link_color = if port.status == "connected" { theme::SUCCESS } else { theme::TEXT_DISABLED };
        ui.label(link_x + 4.0, row_r.y + 10.0, link_str, theme::FONT_SIZE_SMALL, link_color);

        // Color overlay: board cross-ref (highlight if matched)
        if port.board_id.is_some() {
            let board_x = link_x + widths[5] + widths[6];
            ui.label(board_x + 4.0, row_r.y + 10.0, &board_label, theme::FONT_SIZE_SMALL, theme::ACCENT);
        }

        y += theme::ROW_HEIGHT;
    }
}

// ---- Helpers ----

/// Get background color for a board card based on its state.
fn board_state_bg(board: &crate::state::BoardState) -> theme::Color {
    if !board.alive {
        return theme::BG_SECONDARY;
    }
    match board.status.as_ref().map(|s| &s.state) {
        Some(fbc_host::types::ControllerState::Running) => theme::BG_TERTIARY,
        Some(fbc_host::types::ControllerState::Error) => theme::Color::rgb(0.15, 0.05, 0.05),
        Some(fbc_host::types::ControllerState::Done) => theme::Color::rgb(0.05, 0.08, 0.15),
        _ => theme::BG_SECONDARY,
    }
}

/// Get state label and color for a board.
fn board_state_label(board: &crate::state::BoardState) -> (String, theme::Color) {
    if !board.alive {
        return ("Offline".into(), theme::TEXT_DISABLED);
    }
    match board.status.as_ref().map(|s| &s.state) {
        Some(fbc_host::types::ControllerState::Idle) => ("Idle".into(), theme::IDLE),
        Some(fbc_host::types::ControllerState::Running) => ("Running".into(), theme::SUCCESS),
        Some(fbc_host::types::ControllerState::Done) => ("Done".into(), theme::ACCENT),
        Some(fbc_host::types::ControllerState::Error) => ("Error".into(), theme::ERROR),
        None => ("Ready".into(), theme::TEXT_SECONDARY),
    }
}

/// Color for temperature value: green <80C, yellow 80-120C, red >120C.
fn temp_color(temp_c: f32) -> theme::Color {
    if temp_c > 120.0 {
        theme::ERROR
    } else if temp_c > 80.0 {
        theme::WARNING
    } else {
        theme::SUCCESS
    }
}

/// Truncate address string for card display.
fn truncate_addr(addr: &str, max: usize) -> String {
    if addr.len() <= max {
        addr.to_string()
    } else {
        format!("{}...", &addr[..max - 3])
    }
}

/// Derive Sonoma slot info from IP address.
/// 172.16.0.101-144 = Front Slot 1-44, 172.16.0.201-244 = Rear Slot 1-44.
/// Each slot: shelf = (slot-1)/4 + 1, position = front/rear.
fn sonoma_slot_info(ip: &str) -> (String, String, String) {
    if let Some(last_octet) = ip.rsplit('.').next().and_then(|s| s.parse::<u32>().ok()) {
        if last_octet >= 101 && last_octet <= 144 {
            let slot = last_octet - 100;
            let shelf = (slot - 1) / 4 + 1;
            return (format!("F{}", slot), format!("{}", shelf), "Front".into());
        } else if last_octet >= 201 && last_octet <= 244 {
            let slot = last_octet - 200;
            let shelf = (slot - 1) / 4 + 1;
            return (format!("R{}", slot), format!("{}", shelf), "Rear".into());
        }
    }
    ("?".into(), "?".into(), "?".into())
}

/// Rough NTC temperature estimate from voltage (mV) for display purposes.
/// Uses simplified 30K NTC B=3985.3 with 4.98K pulldown topology.
/// Returns degrees C. For accurate values, use the firmware's B-equation.
fn ntc_estimate(voltage_mv: f32) -> f32 {
    if voltage_mv <= 0.0 || voltage_mv >= 3300.0 {
        return -999.0;
    }
    let v = voltage_mv / 1000.0;
    let v_ref = 3.3;
    let r_pull = 4980.0;
    // NTC on top: R_ntc = R_pull * V / (Vref - V)
    let r_ntc = r_pull * v / (v_ref - v);
    if r_ntc <= 0.0 {
        return -999.0;
    }
    let b = 3985.3_f32;
    let r0 = 30000.0_f32;
    let t0 = 298.15_f32; // 25C in Kelvin
    // B-equation: 1/T = 1/T0 + (1/B) * ln(R/R0)
    let inv_t = 1.0 / t0 + (1.0 / b) * (r_ntc / r0).ln();
    if inv_t <= 0.0 {
        return -999.0;
    }
    (1.0 / inv_t) - 273.15
}
