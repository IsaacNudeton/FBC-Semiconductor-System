/// Vectors panel — profile-separated vector operations.
/// FBC: .fbc compressed vectors, uploaded via DMA (HwCommand::UploadVectors).
/// Sonoma: .hex + .seq files, loaded via SSH (SonomaLoadVectors + SonomaRunVectors).
/// Uses orchestration target to select WHICH boards within each profile get vectors.

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
    ui.label(header.x, header.y + 8.0, "Vector Engine", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    // Target selector — shared, each section filters by profile
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
    let sonoma_ips: Vec<String> = all_targets.iter()
        .filter_map(|id| match id { BoardId::Ip(ip) => Some(ip.clone()), _ => None })
        .collect();

    // Summary
    let summary = col.next(24.0);
    ui.label(summary.x, summary.y + 4.0,
        &format!("Target: {} FBC / {} Sonoma board(s)", fbc_targets.len(), sonoma_ips.len()),
        theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

    // Vector status card (shows selected board's data — FBC has real-time status)
    let status_area = col.next(180.0);
    let inner = ui.card(status_area, "Engine Status");

    let vs = state.selected_board_state().and_then(|b| b.vector_status.as_ref());

    if let Some(vs) = vs {
        let mut scol = Column::new(inner).with_gap(8.0);

        // State badge + refresh
        let state_row = scol.next(28.0);
        let (state_label, state_color) = match vs.state {
            fbc_host::types::VectorState::Idle => ("IDLE", theme::IDLE),
            fbc_host::types::VectorState::Loading => ("LOADING", theme::WARNING),
            fbc_host::types::VectorState::Running => ("RUNNING", theme::SUCCESS),
            fbc_host::types::VectorState::Paused => ("PAUSED", theme::WARNING),
            fbc_host::types::VectorState::Done => ("DONE", theme::ACCENT),
            fbc_host::types::VectorState::Error => ("ERROR", theme::ERROR),
        };
        ui.badge(state_row.x, state_row.y, state_label, state_color);

        let refresh_r = Rect::new(state_row.x + 100.0, state_row.y, 80.0, 26.0);
        if ui.button_ghost(refresh_r, "Refresh") {
            if let Some(id) = state.selected_board.clone() {
                state.send_command(HwCommand::GetVectorStatus(id));
            }
        }

        // Progress bar
        let prog_row = scol.next(24.0);
        let progress = if vs.total_vectors > 0 {
            vs.current_address as f32 / vs.total_vectors as f32
        } else {
            0.0
        };
        let prog_label = format!("{}/{} vectors ({:.1}%)", vs.current_address, vs.total_vectors, progress * 100.0);
        ui.progress_labeled(prog_row, progress, theme::ACCENT, &prog_label);

        // Loop progress
        let loop_row = scol.next(24.0);
        let loop_progress = if vs.target_loops > 0 {
            vs.loop_count as f32 / vs.target_loops as f32
        } else {
            0.0
        };
        let loop_label = format!("Loop {}/{}", vs.loop_count, vs.target_loops);
        ui.progress_labeled(loop_row, loop_progress, theme::SUCCESS, &loop_label);

        // Stats
        let stats_row = scol.next(20.0);
        let errors_color = if vs.error_count > 0 { theme::ERROR } else { theme::TEXT_SECONDARY };
        ui.label(stats_row.x, stats_row.y, &format!("Errors: {}", vs.error_count), theme::FONT_SIZE_NORMAL, errors_color);

        let time_str = format!("Time: {:.1}s", vs.run_time_ms as f64 / 1000.0);
        ui.label(stats_row.x + 200.0, stats_row.y, &time_str, theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
    } else {
        ui.label(inner.x, inner.y + 8.0, "No vector engine data. Select a board and click Refresh.", theme::FONT_SIZE_NORMAL, theme::TEXT_SECONDARY);
    }

    // Separator
    col.next(1.0);
    ui.separator(col.next(1.0));

    // ---- FBC Vectors Section ----
    let fbc_area = col.next(130.0);
    let fbc_inner = ui.card(fbc_area, "FBC Vectors (.fbc)");

    let fbc_count = fbc_targets.len();
    let fbc_count_color = if fbc_count > 0 { theme::TEXT_PRIMARY } else { theme::TEXT_DISABLED };
    ui.label(fbc_inner.x, fbc_inner.y,
        &format!("{} FBC board(s) targeted | Compressed .fbc format, DMA upload", fbc_count),
        theme::FONT_SIZE_SMALL, fbc_count_color);

    // FBC controls row
    let fbc_ctrl_row = Rect::new(fbc_inner.x, fbc_inner.y + 24.0, fbc_inner.w, 36.0);
    let mut fbc_row = Row::new(fbc_ctrl_row).with_gap(8.0);

    if fbc_count > 0 {
        // Upload .fbc
        let upload_r = fbc_row.next(120.0);
        if ui.button(upload_r, "Upload .fbc") {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("FBC Vectors", &["fbc"])
                .pick_file()
            {
                match std::fs::read(&path) {
                    Ok(data) => {
                        let size = data.len();
                        for id in &fbc_targets {
                            state.send_command(HwCommand::UploadVectors(id.clone(), data.clone()));
                        }
                        state.status_message = format!(
                            "Uploading .fbc ({} bytes) to {} FBC board(s)",
                            size, fbc_count
                        );
                    }
                    Err(e) => state.status_message = format!("Read error: {}", e),
                }
            }
        }

        let start_r = fbc_row.next(70.0);
        if ui.button(start_r, "Start") {
            for id in &fbc_targets {
                state.send_command(HwCommand::StartVectors(id.clone(), 1));
            }
        }

        let pause_r = fbc_row.next(70.0);
        if ui.button_ghost(pause_r, "Pause") {
            for id in &fbc_targets {
                state.send_command(HwCommand::PauseVectors(id.clone()));
            }
        }

        let resume_r = fbc_row.next(70.0);
        if ui.button_ghost(resume_r, "Resume") {
            for id in &fbc_targets {
                state.send_command(HwCommand::ResumeVectors(id.clone()));
            }
        }

        let stop_r = fbc_row.next(70.0);
        if ui.button_colored(stop_r, "Stop", theme::ERROR, theme::EMERGENCY) {
            for id in &fbc_targets {
                state.send_command(HwCommand::StopVectors(id.clone()));
            }
        }
    } else {
        ui.label(fbc_ctrl_row.x, fbc_ctrl_row.y + 8.0,
            "No FBC boards in current target",
            theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
    }

    // FBC board list
    let fbc_list_y = fbc_inner.y + 68.0;
    if fbc_count > 0 {
        let names: Vec<String> = fbc_targets.iter().take(8).map(|id| {
            match id { BoardId::Mac(mac) => fbc_host::format_mac(mac), BoardId::Ip(ip) => ip.clone() }
        }).collect();
        let suffix = if fbc_count > 8 { format!(" +{}", fbc_count - 8) } else { String::new() };
        ui.label(fbc_inner.x, fbc_list_y,
            &format!("-> {}{}", names.join(", "), suffix),
            theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
    }

    // ---- Sonoma Vectors Section ----
    let sonoma_area = col.next(130.0);
    let sonoma_inner = ui.card(sonoma_area, "Sonoma Vectors (.hex + .seq)");

    let sonoma_count = sonoma_ips.len();
    let sonoma_count_color = if sonoma_count > 0 { theme::TEXT_PRIMARY } else { theme::TEXT_DISABLED };
    ui.label(sonoma_inner.x, sonoma_inner.y,
        &format!("{} Sonoma board(s) targeted | .hex + .seq via SSH load + run", sonoma_count),
        theme::FONT_SIZE_SMALL, sonoma_count_color);

    let sonoma_ctrl_row = Rect::new(sonoma_inner.x, sonoma_inner.y + 24.0, sonoma_inner.w, 36.0);
    let mut s_row = Row::new(sonoma_ctrl_row).with_gap(8.0);

    if sonoma_count > 0 {
        // Load vectors
        let load_r = s_row.next(140.0);
        if ui.button(load_r, "Load Vectors") {
            for ip in &sonoma_ips {
                state.send_command(HwCommand::SonomaLoadVectors(
                    ip.clone(),
                    "/home/device/test.seq".into(),
                    "/home/device/test.hex".into(),
                ));
            }
            state.status_message = format!("Loading vectors on {} Sonoma board(s)...", sonoma_count);
        }

        // Run
        let run_r = s_row.next(100.0);
        if ui.button(run_r, "Run") {
            for ip in &sonoma_ips {
                state.send_command(HwCommand::SonomaRunVectors(
                    ip.clone(),
                    "/home/device/test.seq".into(),
                    60,
                    false,
                ));
            }
            state.status_message = format!("Running vectors on {} Sonoma board(s)...", sonoma_count);
        }

        // Stop
        let estop_r = s_row.next(80.0);
        if ui.button_colored(estop_r, "Stop", theme::ERROR, theme::EMERGENCY) {
            for ip in &sonoma_ips {
                state.send_command(HwCommand::SonomaEmergencyStop(ip.clone()));
            }
        }
    } else {
        ui.label(sonoma_ctrl_row.x, sonoma_ctrl_row.y + 8.0,
            "No Sonoma boards in current target",
            theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
    }

    // Sonoma board list
    let sonoma_list_y = sonoma_inner.y + 68.0;
    if sonoma_count > 0 {
        let names: Vec<&str> = sonoma_ips.iter().take(8).map(|s| s.as_str()).collect();
        let suffix = if sonoma_count > 8 { format!(" +{}", sonoma_count - 8) } else { String::new() };
        ui.label(sonoma_inner.x, sonoma_list_y,
            &format!("-> {}{}", names.join(", "), suffix),
            theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
    }
}
