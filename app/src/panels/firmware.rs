/// Firmware panel — profile-separated firmware update.
/// FBC: BOOT.BIN (bitstream + bare-metal ELF) via FBC protocol chunked transfer.
/// Sonoma: firmware package (28 ELFs + scripts) via SCP to /mnt/bin/.
/// These are COMPLETELY DIFFERENT binaries — never cross-upload.
/// Uses orchestration target to select WHICH boards within each profile get flashed.

use crate::ui::Ui;
use crate::layout::{Rect, Column, Row};
use crate::state::AppState;
use crate::transport::{BoardId, HwCommand};
use crate::theme;

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Firmware Update", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    // Target selector — shared across both sections, filters within each
    let target_row = col.next(32.0);
    if let Some(new_target) = ui.target_selector(target_row, &state.command_target, "Target:") {
        state.command_target = new_target;
    }

    // Resolve targets ONCE, then split by profile
    let all_targets = state.resolve_targets();
    let fbc_targets: Vec<BoardId> = all_targets.iter()
        .filter(|id| matches!(id, BoardId::Mac(_)))
        .cloned()
        .collect();
    let sonoma_targets: Vec<String> = all_targets.iter()
        .filter_map(|id| match id { BoardId::Ip(ip) => Some(ip.clone()), _ => None })
        .collect();

    // Summary
    let summary = col.next(24.0);
    let total_fbc = state.boards.iter().filter(|b| b.is_fbc()).count();
    let total_sonoma = state.boards.iter().filter(|b| b.is_sonoma()).count();
    ui.label(summary.x, summary.y + 4.0,
        &format!("Target resolves to: {} FBC / {} Sonoma (of {} / {} total)",
            fbc_targets.len(), sonoma_targets.len(), total_fbc, total_sonoma),
        theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    // Show firmware info for the selected board (read-only, works for both)
    let info_area = col.next(180.0);
    let inner = ui.card(info_area, "Current Firmware");

    if let Some(board) = state.selected_board_state() {
        let board_label = board.label.clone();
        let board_type = board.type_label().to_string();
        let fw_ver = board.fw_version.clone();
        let board_id = board.id.clone();

        ui.label(inner.x, inner.y, &format!("{} ({})", board_label, board_type), theme::FONT_SIZE_NORMAL, theme::TEXT_PRIMARY);
        ui.label(inner.x, inner.y + 20.0, &format!("Version: {}", fw_ver), theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

        // Get Info button
        let info_btn = Rect::new(inner.x, inner.y + 44.0, 120.0, 32.0);
        if ui.button(info_btn, "Get Info") {
            state.send_command(HwCommand::GetFirmwareInfo(board_id.clone()));
        }

        // Show FBC-specific firmware info fields
        let fw_info = state.selected_board_state().and_then(|b| b.firmware_info.as_ref());
        if let Some(fw) = fw_info {
            let mut y = inner.y + 84.0;
            let fields = [
                ("Version", format!("{}.{}.{}", fw.version_major, fw.version_minor, fw.version_patch)),
                ("Build Date", fw.build_date.clone()),
                ("Serial", format!("{:08X}", fw.serial)),
                ("HW Revision", format!("{}", fw.hw_revision)),
                ("Bootloader", format!("v{}", fw.bootloader_version)),
                ("SD Present", format!("{}", fw.sd_present)),
                ("Update Active", format!("{}", fw.update_in_progress)),
            ];
            for (label, value) in &fields {
                ui.label(inner.x, y, label, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
                ui.label(inner.x + 140.0, y, value, theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);
                y += 18.0;
            }
        }
    } else {
        ui.label(inner.x, inner.y, "Select a board to view firmware info", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
    }

    // Separator
    col.next(1.0);
    ui.separator(col.next(1.0));

    // ---- FBC Firmware Update Section ----
    let fbc_area = col.next(140.0);
    let fbc_inner = ui.card(fbc_area, "FBC Firmware (BOOT.BIN)");

    let fbc_count = fbc_targets.len();

    // Row: count + upload button
    let fbc_info_row = Rect::new(fbc_inner.x, fbc_inner.y, fbc_inner.w, 18.0);
    let count_color = if fbc_count > 0 { theme::TEXT_PRIMARY } else { theme::TEXT_DISABLED };
    ui.label(fbc_info_row.x, fbc_info_row.y,
        &format!("{} FBC board(s) targeted", fbc_count),
        theme::FONT_SIZE_SMALL, count_color);

    ui.label(fbc_inner.x, fbc_inner.y + 18.0,
        "BOOT.BIN = bitstream + bare-metal ARM ELF. Uploaded via FBC protocol.",
        theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    let fbc_btn_row = Rect::new(fbc_inner.x, fbc_inner.y + 44.0, fbc_inner.w, 36.0);
    let mut fbc_row = Row::new(fbc_btn_row).with_gap(8.0);

    if fbc_count > 0 {
        let upload_r = fbc_row.next(160.0);
        if ui.button(upload_r, "Select BOOT.BIN") {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("BOOT.BIN", &["bin", "BIN"])
                .add_filter("Bitstream", &["bit"])
                .pick_file()
            {
                match std::fs::read(&path) {
                    Ok(data) => {
                        let crc = data.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
                        let size = data.len();
                        for id in &fbc_targets {
                            state.send_command(HwCommand::FirmwareUpdate(id.clone(), data.clone(), crc));
                        }
                        state.status_message = format!(
                            "Uploading BOOT.BIN ({} bytes) to {} FBC board(s)...",
                            size, fbc_count
                        );
                    }
                    Err(e) => state.status_message = format!("Read error: {}", e),
                }
            }
        }

        // Show which boards will receive the update
        let list_r = fbc_row.remaining();
        let names: Vec<String> = fbc_targets.iter().take(5).map(|id| {
            match id { BoardId::Mac(mac) => fbc_host::format_mac(mac), BoardId::Ip(ip) => ip.clone() }
        }).collect();
        let suffix = if fbc_targets.len() > 5 { format!(" +{}", fbc_targets.len() - 5) } else { String::new() };
        ui.label(list_r.x, list_r.y + 8.0,
            &format!("{}{}", names.join(", "), suffix),
            theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
    } else {
        ui.label(fbc_btn_row.x, fbc_btn_row.y + 8.0,
            "No FBC boards in current target. Change target above.",
            theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
    }

    // FBC progress
    let fbc_prog = Rect::new(fbc_inner.x, fbc_inner.y + 84.0, fbc_inner.w, 20.0);
    ui.progress_labeled(fbc_prog, 0.0, theme::ACCENT, "Ready");

    // ---- Sonoma Firmware Update Section ----
    let sonoma_area = col.next(140.0);
    let sonoma_inner = ui.card(sonoma_area, "Sonoma Firmware (SCP)");

    let sonoma_count = sonoma_targets.len();

    let sonoma_count_color = if sonoma_count > 0 { theme::TEXT_PRIMARY } else { theme::TEXT_DISABLED };
    ui.label(sonoma_inner.x, sonoma_inner.y,
        &format!("{} Sonoma board(s) targeted", sonoma_count),
        theme::FONT_SIZE_SMALL, sonoma_count_color);

    ui.label(sonoma_inner.x, sonoma_inner.y + 18.0,
        "Sonoma = Linux + 28 ELFs on SD. Firmware package deployed via SCP.",
        theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    let sonoma_btn_row = Rect::new(sonoma_inner.x, sonoma_inner.y + 44.0, sonoma_inner.w, 36.0);
    let mut s_row = Row::new(sonoma_btn_row).with_gap(8.0);

    if sonoma_count > 0 {
        let upload_r = s_row.next(180.0);
        if ui.button(upload_r, "Select FW Package") {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Firmware", &["tar", "gz", "zip"])
                .add_filter("All", &["*"])
                .pick_file()
            {
                let path_str = path.display().to_string();
                for ip in &sonoma_targets {
                    state.send_command(HwCommand::SonomaUpdateFirmware(ip.clone(), path_str.clone()));
                }
                state.status_message = format!(
                    "Deploying firmware to {} Sonoma board(s)...",
                    sonoma_count
                );
            }
        }

        // Show which boards
        let list_r = s_row.remaining();
        let names: Vec<&str> = sonoma_targets.iter().take(5).map(|s| s.as_str()).collect();
        let suffix = if sonoma_targets.len() > 5 { format!(" +{}", sonoma_targets.len() - 5) } else { String::new() };
        ui.label(list_r.x, list_r.y + 8.0,
            &format!("{}{}", names.join(", "), suffix),
            theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
    } else {
        ui.label(sonoma_btn_row.x, sonoma_btn_row.y + 8.0,
            "No Sonoma boards in current target. Change target above.",
            theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
    }

    // Sonoma progress
    let sonoma_prog = Rect::new(sonoma_inner.x, sonoma_inner.y + 84.0, sonoma_inner.w, 20.0);
    ui.progress_labeled(sonoma_prog, 0.0, theme::SUCCESS, "Ready");
}
