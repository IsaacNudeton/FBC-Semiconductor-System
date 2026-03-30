/// FBC Semiconductor System — Native wgpu GUI
/// 4-tab architecture: Dashboard, Profiling, Engineering, Datalogs
/// Persistent sidebar with board tree (System → Shelf → Tray → Board)
/// ONE loop, TWO I/O channels (pixels out, hardware bytes in/out)

#[allow(dead_code)]
mod gpu;
#[allow(dead_code)]
mod text;
#[allow(dead_code)]
mod draw;
#[allow(dead_code)]
mod layout;
mod input;
#[allow(dead_code)]
mod ui;
#[allow(dead_code)]
mod theme;
#[allow(dead_code)]
mod state;
mod transport;
mod panels;
#[allow(dead_code)]
mod pattern_converter;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId, WindowAttributes};
use winit::dpi::LogicalSize;
use tokio::sync::mpsc;
use std::sync::Arc;

use crate::gpu::Gpu;
use crate::text::TextRenderer;
use crate::draw::DrawList;
use crate::input::InputState;
use crate::ui::Ui;
use crate::layout::{Rect, Column};
use crate::state::{AppState, Tab};
use crate::transport::{BoardId, HwCommand, HwResponse};

// Embedded font — JetBrains Mono Regular
const FONT_DATA: &[u8] = include_bytes!("../assets/font.ttf");

struct App {
    window: Option<Arc<Window>>,
    gpu: Option<Gpu>,
    text: Option<TextRenderer>,
    draw_list: DrawList,
    input: InputState,
    state: Option<AppState>,
    rsp_rx: Option<mpsc::Receiver<HwResponse>>,
}

impl App {
    fn new(cmd_tx: mpsc::Sender<HwCommand>, rsp_rx: mpsc::Receiver<HwResponse>) -> Self {
        Self {
            window: None,
            gpu: None,
            text: None,
            draw_list: DrawList::new(),
            input: InputState::new(),
            state: Some(AppState::new(cmd_tx)),
            rsp_rx: Some(rsp_rx),
        }
    }

    fn draw_frame(&mut self) {
        let gpu = self.gpu.as_ref().unwrap();
        let text = self.text.as_ref().unwrap();
        let state = self.state.as_mut().unwrap();

        state.frame_count += 1;
        self.input.begin_frame();
        self.draw_list.clear();

        let screen = Rect::new(0.0, 0.0, gpu.width as f32, gpu.height as f32);

        // ---- Layout: sidebar + content ----
        let sidebar_w = if state.sidebar_collapsed {
            theme::SIDEBAR_COLLAPSED_WIDTH
        } else {
            theme::SIDEBAR_WIDTH
        };

        let sidebar_rect = screen.take_left(sidebar_w);
        let content_rect = screen.shrink_left(sidebar_w);

        // ---- Draw sidebar (board tree + alerts) ----
        {
            let mut ui = Ui::new(&mut self.draw_list, text, &self.input);

            // Sidebar background + border
            ui.draw.rect(sidebar_rect, theme::BG_SECONDARY);
            ui.draw.rect(
                Rect::new(sidebar_rect.right() - 1.0, sidebar_rect.y, 1.0, sidebar_rect.h),
                theme::BORDER,
            );

            let mut col = Column::new(sidebar_rect.padded(4.0)).with_gap(2.0);

            // App title
            let title_r = col.next(40.0);
            ui.label(title_r.x + 8.0, title_r.y + 10.0, "FBC System", theme::FONT_SIZE_LARGE, theme::ACCENT);

            // Connection status badge
            let status_text = if state.connected { "Online" } else { "Offline" };
            let status_color = if state.connected { theme::SUCCESS } else { theme::TEXT_DISABLED };
            ui.badge(title_r.x + 130.0, title_r.y + 14.0, status_text, status_color);

            col.next(2.0);
            ui.separator(col.next(1.0));
            col.next(2.0);

            // ---- Board Tree ----
            let tree_label = col.next(20.0);
            ui.label(tree_label.x + 8.0, tree_label.y + 2.0, "BOARDS", theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);

            // Board count summary
            let fbc_count = state.boards.iter().filter(|b| b.is_fbc()).count();
            let sonoma_count = state.boards.iter().filter(|b| b.is_sonoma()).count();
            let total = state.boards.len();
            if total > 0 {
                let summary = col.next(16.0);
                ui.label(summary.x + 8.0, summary.y,
                    &format!("{} total ({} FBC, {} Sonoma)", total, fbc_count, sonoma_count),
                    theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
            }

            // Build shelf list from board_slots
            let max_shelf = state.max_shelf();

            // Collect shelf data before drawing (avoid borrow issues)
            let shelf_data: Vec<(u8, Vec<(BoardId, String, bool, bool, bool)>)> = (1..=max_shelf.max(1)).map(|shelf_num| {
                let boards_on = state.boards_on_shelf(shelf_num);
                let board_info: Vec<_> = boards_on.iter().map(|(b, slot)| {
                    let label = format!("{} B{}", slot.tray.chars().next().unwrap_or('?'), slot.position);
                    let is_selected = state.selected_board.as_ref() == Some(&b.id);
                    let has_error = b.status.as_ref().map_or(false, |s| s.errors > 0);
                    (b.id.clone(), label, b.alive, is_selected, has_error)
                }).collect();
                (shelf_num, board_info)
            }).collect();

            let expanded_shelves = state.expanded_shelves.clone();

            // Scroll area for board tree
            // Render shelf tree
            for (shelf_num, boards) in &shelf_data {
                if col.remaining_height() < 28.0 { break; }

                let expanded = expanded_shelves.contains(shelf_num);
                let shelf_r = col.next(26.0);
                let arrow = if expanded { "v" } else { ">" };
                let shelf_label = format!("{} Shelf {}", arrow, shelf_num);

                // Shelf row — click to expand/collapse
                let hovered = self.input.hovered(shelf_r.x, shelf_r.y, shelf_r.w, shelf_r.h);
                if hovered {
                    ui.draw.rounded_rect(shelf_r, 3.0, theme::BG_HOVER);
                }
                ui.label(shelf_r.x + 8.0, shelf_r.y + 5.0, &shelf_label, theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);

                // Board count badge on shelf row
                let board_count_on_shelf = boards.len();
                if board_count_on_shelf > 0 {
                    let count_text = format!("{}", board_count_on_shelf);
                    ui.badge(shelf_r.x + sidebar_w - 48.0, shelf_r.y + 3.0, &count_text, theme::TEXT_SECONDARY);
                }

                if self.input.clicked_in(shelf_r.x, shelf_r.y, shelf_r.w, shelf_r.h) {
                    if expanded {
                        state.expanded_shelves.remove(shelf_num);
                    } else {
                        state.expanded_shelves.insert(*shelf_num);
                    }
                }

                // Board entries (if expanded)
                if expanded {
                    for entry in boards.iter() {
                        let (ref id, ref label, alive, is_selected, has_error) = *entry;
                        if col.remaining_height() < 24.0 { break; }
                        let board_r = col.next(22.0);

                        // Selection highlight
                        if is_selected {
                            ui.draw.rounded_rect(board_r, 3.0, theme::ACCENT_DIM);
                            ui.draw.rect(Rect::new(board_r.x, board_r.y, 3.0, board_r.h), theme::ACCENT);
                        } else if self.input.hovered(board_r.x, board_r.y, board_r.w, board_r.h) {
                            ui.draw.rounded_rect(board_r, 3.0, theme::BG_HOVER);
                        }

                        // Status dot
                        let dot_color = if has_error {
                            theme::LED_ERROR
                        } else if alive {
                            theme::LED_RUNNING
                        } else {
                            theme::LED_DISCONNECTED
                        };
                        ui.draw.rounded_rect(
                            Rect::new(board_r.x + 20.0, board_r.y + 7.0, 8.0, 8.0),
                            4.0, dot_color,
                        );

                        // Board label
                        let text_color = if is_selected { theme::ACCENT } else { theme::TEXT_SECONDARY };
                        ui.label(board_r.x + 34.0, board_r.y + 3.0, label, theme::FONT_SIZE_SMALL, text_color);

                        // Click to select board
                        if self.input.clicked_in(board_r.x, board_r.y, board_r.w, board_r.h) {
                            state.selected_board = Some(id.clone());
                        }
                    }
                }
            }

            // If no boards discovered, show hint
            if total == 0 {
                let hint_r = col.next(40.0);
                ui.label(hint_r.x + 8.0, hint_r.y + 4.0, "No boards", theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
                ui.label(hint_r.x + 8.0, hint_r.y + 20.0, "Use Dashboard to discover", theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
            }

            // ---- Alerts section (bottom of sidebar) ----
            // Find boards with errors
            let error_boards: Vec<String> = state.boards.iter()
                .filter(|b| b.status.as_ref().map_or(false, |s| s.errors > 0))
                .take(5)
                .map(|b| {
                    let errs = b.status.as_ref().map_or(0, |s| s.errors);
                    format!("{}: {} err", b.label, errs)
                })
                .collect();

            let lost_boards: Vec<String> = state.boards.iter()
                .filter(|b| !b.alive)
                .take(3)
                .map(|b| format!("{}: lost", b.label))
                .collect();

            let alert_count = error_boards.len() + lost_boards.len();

            // Bottom section — alerts + status
            let bottom_h = 80.0 + (alert_count.min(5) as f32 * 16.0);
            let bottom = Rect::new(
                sidebar_rect.x + 4.0,
                sidebar_rect.bottom() - bottom_h,
                sidebar_w - 8.0,
                bottom_h,
            );
            ui.draw.rect(Rect::new(bottom.x, bottom.y, bottom.w, 1.0), theme::BORDER);

            let mut by = bottom.y + 8.0;

            // Alert header
            if alert_count > 0 {
                let alert_label = format!("ALERTS ({})", alert_count);
                ui.label(bottom.x + 8.0, by, &alert_label, theme::FONT_SIZE_SMALL, theme::WARNING);
                by += 18.0;

                for msg in error_boards.iter().chain(lost_boards.iter()).take(5) {
                    ui.label(bottom.x + 12.0, by, msg, theme::FONT_SIZE_SMALL, theme::ERROR);
                    by += 16.0;
                }
            }

            // Selected board
            let board_label = state.selected_board.as_ref()
                .map(|id| AppState::board_label(id))
                .unwrap_or_else(|| "No board selected".into());
            ui.label(bottom.x + 8.0, by + 4.0, &board_label, theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);

            // Status message
            let status_msg = state.status_message.chars().take(30).collect::<String>();
            ui.label(bottom.x + 8.0, by + 20.0, &status_msg, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
        }

        // ---- Draw content area (tab bar + sub-panel tabs + panel) ----
        {
            let mut ui = Ui::new(&mut self.draw_list, text, &self.input);

            // Content background
            ui.draw.rect(content_rect, theme::BG_PRIMARY);

            // Tab bar at top (4 main tabs)
            let tab_bar_h = 40.0;
            let tab_bar = content_rect.take_top(tab_bar_h);
            ui.draw.rect(tab_bar, theme::BG_SECONDARY);
            ui.draw.rect(Rect::new(tab_bar.x, tab_bar.bottom() - 1.0, tab_bar.w, 1.0), theme::BORDER);

            // Draw 4 main tabs
            let tab_labels: Vec<&str> = Tab::ALL.iter().map(|t| t.label()).collect();
            let active_tab_idx = Tab::ALL.iter().position(|t| *t == state.active_tab).unwrap_or(0);
            if let Some(clicked_idx) = ui.tabs(tab_bar, &tab_labels, active_tab_idx) {
                state.switch_tab(Tab::ALL[clicked_idx]);
            }

            // Sub-panel tab bar (panels within the active tab)
            let sub_views = state.active_tab.sub_views();
            let below_tabs = Rect::new(
                content_rect.x,
                content_rect.y + tab_bar_h,
                content_rect.w,
                content_rect.h - tab_bar_h,
            );

            if sub_views.len() > 1 {
                // Draw sub-panel tabs
                let sub_tab_h = 32.0;
                let sub_tab_bar = below_tabs.take_top(sub_tab_h);
                ui.draw.rect(sub_tab_bar, theme::BG_TERTIARY);

                let sub_labels: Vec<&str> = sub_views.iter().map(|v| v.label()).collect();
                let active_sub_idx = sub_views.iter().position(|v| *v == state.active_view).unwrap_or(0);
                if let Some(clicked_sub) = ui.tabs(sub_tab_bar, &sub_labels, active_sub_idx) {
                    state.switch_sub_view(sub_views[clicked_sub]);
                }

                // Panel area below sub-tabs
                let panel_rect = Rect::new(
                    below_tabs.x,
                    below_tabs.y + sub_tab_h,
                    below_tabs.w,
                    below_tabs.h - sub_tab_h,
                );
                panels::draw_panel(&mut ui, panel_rect, state);
            } else {
                // Single sub-panel (Datalogs) — no sub-tab bar
                panels::draw_panel(&mut ui, below_tabs, state);
            }
        }

        // ---- Render ----
        let output = match gpu.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                let gpu = self.gpu.as_mut().unwrap();
                gpu.resize(gpu.width, gpu.height);
                return;
            }
            Err(e) => {
                eprintln!("Surface error: {}", e);
                return;
            }
        };

        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("frame"),
        });

        // Clear pass
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.051, g: 0.067, b: 0.09, a: 1.0, // #0d1117
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
        }

        // 2D UI pass
        self.draw_list.render(gpu, text, &mut encoder, &view);

        gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }

        let attrs = WindowAttributes::default()
            .with_title("FBC Semiconductor System")
            .with_inner_size(LogicalSize::new(1920, 1080));

        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));

        let gpu = Gpu::new(window.clone());
        let text = TextRenderer::new(&gpu, FONT_DATA);

        self.window = Some(window);
        self.gpu = Some(gpu);
        self.text = Some(text);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.input.handle_event(&event);

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }

            WindowEvent::Resized(size) => {
                if let Some(gpu) = self.gpu.as_mut() {
                    gpu.resize(size.width, size.height);
                }
            }

            WindowEvent::RedrawRequested => {
                // Drain hardware responses
                if let Some(rsp_rx) = self.rsp_rx.as_mut() {
                    while let Ok(rsp) = rsp_rx.try_recv() {
                        if let Some(state) = self.state.as_mut() {
                            state.handle_response(rsp);
                        }
                    }
                }

                self.draw_frame();

                if let Some(window) = self.window.as_ref() {
                    window.request_redraw();
                }
            }

            _ => {}
        }
    }
}

fn main() {
    // Hardware I/O channels
    let (cmd_tx, cmd_rx) = mpsc::channel::<HwCommand>(256);
    let (rsp_tx, rsp_rx) = mpsc::channel::<HwResponse>(256);

    // Spawn hardware I/O on its own thread with tokio runtime
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
        rt.block_on(transport::hardware_loop(cmd_rx, rsp_tx));
    });

    // Run the window event loop (blocks)
    let event_loop = EventLoop::new().expect("event loop");
    let mut app = App::new(cmd_tx, rsp_rx);
    event_loop.run_app(&mut app).expect("run app");
}
