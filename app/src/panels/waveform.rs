/// Waveform panel — pin-level digital waveform viewer (logic analyzer style)
/// Displays vector data as digital waveforms per pin, with zoom/scroll/tab controls.

use crate::ui::Ui;
use crate::layout::{Rect, Column, Row};
use crate::state::AppState;
use crate::theme;

const PIN_LABEL_WIDTH: f32 = 120.0;
const LANE_HEIGHT: f32 = 24.0;
const VISIBLE_LANES: usize = 16;
const RULER_HEIGHT: f32 = 20.0;
const TRANSITION_WIDTH: f32 = 2.0;

/// Pin type for display badges
#[derive(Clone, Copy)]
enum PinKind {
    IO,
    Pulse,
    Monitor,
    Supply,
}

impl PinKind {
    fn label(self) -> &'static str {
        match self {
            PinKind::IO => "I/O",
            PinKind::Pulse => "PULSE",
            PinKind::Monitor => "MON",
            PinKind::Supply => "SUP",
        }
    }

    fn color(self) -> theme::Color {
        match self {
            PinKind::IO => theme::ACCENT,
            PinKind::Pulse => theme::WARNING,
            PinKind::Monitor => theme::SUCCESS,
            PinKind::Supply => theme::ERROR,
        }
    }

    /// Placeholder assignment based on pin index (until real pin config loaded)
    fn from_index(pin: usize) -> Self {
        if pin >= 160 {
            PinKind::Supply
        } else if pin >= 128 {
            // Fast pins: first 4 are pulse clocks, rest are I/O
            if pin < 132 { PinKind::Pulse } else { PinKind::IO }
        } else {
            // BIM pins: last 8 are monitors, rest are I/O
            if pin >= 120 { PinKind::Monitor } else { PinKind::IO }
        }
    }
}

pub fn draw(ui: &mut Ui, rect: Rect, state: &mut AppState) {
    let content = rect.padded(theme::PADDING);
    let mut col = Column::new(content).with_gap(12.0);

    // ── Header ──
    let header = col.next(48.0);
    ui.label(header.x, header.y + 8.0, "Waveform Viewer", theme::FONT_SIZE_TITLE, theme::TEXT_PRIMARY);

    // ── Controls row ──
    let controls = col.next(44.0);
    let mut ctrl_row = Row::new(controls).with_gap(8.0);

    // Load Vectors button
    let load_r = ctrl_row.next(120.0);
    if ui.button(load_r, "Load Vectors") {
        state.status_message = "Use CLI: fbc-cli fbc upload <file.fbc>".into();
    }

    // Zoom slider
    let zoom_r = ctrl_row.next(200.0);
    let zoom_key = "waveform_zoom";
    let current_zoom = state.get_float(zoom_key).unwrap_or(1.0);
    if let Some(new_zoom) = ui.slider(zoom_r, "Zoom", current_zoom, 1.0, 32.0) {
        state.set_float(zoom_key, new_zoom);
    }
    let zoom = state.get_float(zoom_key).unwrap_or(1.0);

    // Time offset display
    let offset_r = ctrl_row.remaining();
    let scroll_key = "waveform_scroll";
    let scroll_offset = state.get_float(scroll_key).unwrap_or(0.0) as u32;
    ui.label(
        offset_r.x + 8.0,
        offset_r.y + 14.0,
        &format!("T offset: {} vectors", scroll_offset),
        theme::FONT_SIZE_NORMAL,
        theme::TEXT_SECONDARY,
    );

    // ── Tabs ──
    let tab_r = col.next(36.0);
    let tab_labels = ["Fast Pins (128-159)", "BIM Pins (0-127)", "All Pins"];
    let active_tab = state.tab_index("waveform");
    if let Some(idx) = ui.tabs(tab_r, &tab_labels, active_tab) {
        state.set_tab_index("waveform", idx);
    }
    let active_tab = state.tab_index("waveform");

    // Determine pin range from active tab
    let (pin_start, pin_end) = match active_tab {
        0 => (128usize, 160usize),
        1 => (0, 128),
        _ => (0, 160),
    };
    let total_pins = pin_end - pin_start;

    // Pin scroll index (which pin is at top of the visible window)
    let pin_scroll_key = "waveform_pin_scroll";
    let pin_scroll = (state.get_float(pin_scroll_key).unwrap_or(0.0) as usize)
        .min(total_pins.saturating_sub(VISIBLE_LANES));

    // ── Split layout: pin labels | waveform area ──
    let body = col.remaining();
    let pin_col_rect = body.take_left(PIN_LABEL_WIDTH);
    let wave_rect = body.shrink_left(PIN_LABEL_WIDTH + 4.0);

    // Background for pin labels
    ui.draw.rounded_rect(pin_col_rect, theme::BORDER_RADIUS, theme::BG_SECONDARY);
    ui.draw.border(pin_col_rect, 1.0, theme::BORDER);

    // Background for waveform area
    ui.draw.rounded_rect(wave_rect, theme::BORDER_RADIUS, theme::BG_PRIMARY);
    ui.draw.border(wave_rect, 1.0, theme::BORDER);

    // ── Time ruler at top of waveform area ──
    let ruler = Rect::new(wave_rect.x, wave_rect.y, wave_rect.w, RULER_HEIGHT);
    ui.draw.rect(ruler, theme::BG_SECONDARY);

    let step_px = 8.0 * zoom;
    let _visible_vectors = ((wave_rect.w / step_px) as u32).max(1);
    // Draw ruler tick marks every 8 or 16 vectors depending on zoom
    let tick_interval = if zoom >= 4.0 { 4u32 } else if zoom >= 2.0 { 8 } else { 16 };
    {
        let mut vec_idx = scroll_offset;
        let mut px = wave_rect.x;
        while px < wave_rect.right() {
            if vec_idx % tick_interval == 0 {
                // Tick mark
                ui.draw.rect(Rect::new(px, ruler.y + 14.0, 1.0, 6.0), theme::TEXT_DISABLED);
                // Label
                ui.label(px + 2.0, ruler.y + 4.0, &format!("{}", vec_idx), theme::FONT_SIZE_SMALL, theme::TEXT_DISABLED);
            }
            vec_idx += 1;
            px += step_px;
        }
    }

    // ── Pin labels + Waveform lanes ──
    let wave_content_y = wave_rect.y + RULER_HEIGHT;
    let _wave_content_h = wave_rect.h - RULER_HEIGHT;
    let lanes_to_draw = VISIBLE_LANES.min(total_pins - pin_scroll);

    let has_data = false; // No real vector data loaded yet

    for lane in 0..lanes_to_draw {
        let pin_idx = pin_start + pin_scroll + lane;
        let lane_y = wave_content_y + lane as f32 * LANE_HEIGHT;

        // Skip if we'd draw outside the area
        if lane_y + LANE_HEIGHT > wave_rect.bottom() {
            break;
        }

        let kind = PinKind::from_index(pin_idx);

        // ── Pin label (left column) ──
        let label_y = pin_col_rect.y + RULER_HEIGHT + lane as f32 * LANE_HEIGHT;
        if label_y + LANE_HEIGHT <= pin_col_rect.bottom() {
            // Pin name
            let pin_name = format!("PIN_{}", pin_idx);
            ui.label(
                pin_col_rect.x + 4.0,
                label_y + 4.0,
                &pin_name,
                theme::FONT_SIZE_SMALL,
                theme::TEXT_PRIMARY,
            );

            // Type badge
            ui.badge(
                pin_col_rect.x + 68.0,
                label_y + 3.0,
                kind.label(),
                kind.color(),
            );
        }

        // ── Waveform lane ──
        // Lane separator line
        ui.draw.rect(
            Rect::new(wave_rect.x, lane_y + LANE_HEIGHT - 1.0, wave_rect.w, 1.0),
            theme::BG_TERTIARY,
        );

        if !has_data {
            // Placeholder waveform pattern
            draw_placeholder_waveform(ui, wave_rect.x, lane_y, wave_rect.w, pin_idx, zoom);
        }
    }

    // ── Scroll indicators ──
    if total_pins > VISIBLE_LANES {
        // Up arrow region
        if pin_scroll > 0 {
            let up_r = Rect::new(pin_col_rect.x, pin_col_rect.y, pin_col_rect.w, 16.0);
            if ui.button_ghost(up_r, "^ Scroll Up") {
                let new_scroll = (pin_scroll as f32 - 4.0).max(0.0);
                state.set_float(pin_scroll_key, new_scroll);
            }
        }
        // Down arrow region
        if pin_scroll + VISIBLE_LANES < total_pins {
            let dn_r = Rect::new(
                pin_col_rect.x,
                pin_col_rect.bottom() - 16.0,
                pin_col_rect.w,
                16.0,
            );
            if ui.button_ghost(dn_r, "v Scroll Down") {
                let max = (total_pins - VISIBLE_LANES) as f32;
                let new_scroll = (pin_scroll as f32 + 4.0).min(max);
                state.set_float(pin_scroll_key, new_scroll);
            }
        }

        // Pin count indicator
        ui.label(
            wave_rect.right() - 120.0,
            wave_rect.bottom() - 16.0,
            &format!("Pins {}-{} of {}", pin_scroll, pin_scroll + lanes_to_draw, total_pins),
            theme::FONT_SIZE_SMALL,
            theme::TEXT_DISABLED,
        );
    }

    // ── "No data" overlay message ──
    if !has_data {
        let msg = "Load vectors (.hex/.fbc) to view waveforms";
        let msg_x = wave_rect.x + wave_rect.w * 0.5 - 160.0;
        let msg_y = wave_rect.y + wave_rect.h * 0.5 - 8.0;
        // Semi-transparent overlay backdrop
        let overlay = Rect::new(msg_x - 12.0, msg_y - 6.0, 344.0, 28.0);
        ui.draw.rounded_rect(overlay, 4.0, theme::BG_SECONDARY);
        ui.label(msg_x, msg_y, msg, theme::FONT_SIZE_NORMAL, theme::TEXT_DISABLED);
    }
}

/// Draw a placeholder waveform for a single pin lane.
/// Even pins: alternating high/low pattern. Odd pins: steady low with occasional pulse.
fn draw_placeholder_waveform(ui: &mut Ui, x: f32, lane_y: f32, width: f32, pin_idx: usize, zoom: f32) {
    let step_px = 8.0 * zoom;
    let high_y = lane_y + 3.0;
    let low_y = lane_y + LANE_HEIGHT - 7.0;
    let signal_h = 4.0;

    let high_color = theme::ACCENT;
    let low_color = theme::BG_TERTIARY;
    let transition_color = theme::ACCENT_DIM;

    let mut px = x;
    let mut prev_high = false;

    // Pattern period varies by pin to make the display visually interesting
    let period = if pin_idx % 2 == 0 {
        // Even pins: square wave with period based on pin index
        ((pin_idx % 7) + 2) as f32
    } else {
        // Odd pins: mostly low, short pulse every N steps
        ((pin_idx % 11) + 6) as f32
    };

    let mut step = 0u32;
    while px < x + width {
        let seg_w = step_px.min(x + width - px);

        let is_high = if pin_idx % 2 == 0 {
            // Square wave
            (step as f32 % (period * 2.0)) < period
        } else {
            // Short pulse: high for 1 step every `period` steps
            (step as f32 % period) < 1.0
        };

        // Draw signal level
        let (sy, color) = if is_high {
            (high_y, high_color)
        } else {
            (low_y, low_color)
        };
        ui.draw.rect(Rect::new(px, sy, seg_w, signal_h), color);

        // Draw transition (vertical line) when signal changes
        if step > 0 && is_high != prev_high {
            ui.draw.rect(
                Rect::new(px, high_y, TRANSITION_WIDTH, low_y - high_y + signal_h),
                transition_color,
            );
        }

        prev_high = is_high;
        px += step_px;
        step += 1;
    }
}
