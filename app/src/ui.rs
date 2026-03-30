/// Immediate-mode widget API.
/// Every widget is a function: takes state + rect, draws itself, returns interaction.

use crate::draw::DrawList;
use crate::text::TextRenderer;
use crate::input::InputState;
use crate::layout::Rect;
use crate::theme::{self, Color};

pub struct Ui<'a> {
    pub draw: &'a mut DrawList,
    pub text: &'a TextRenderer,
    pub input: &'a InputState,
    /// Hashed ID for tracking which widget has focus
    hot_id: Option<u64>,
}

impl<'a> Ui<'a> {
    pub fn new(draw: &'a mut DrawList, text: &'a TextRenderer, input: &'a InputState) -> Self {
        Self { draw, text, input, hot_id: None }
    }

    // ---- Primitives ----

    pub fn label(&mut self, x: f32, y: f32, text: &str, size: f32, color: Color) {
        self.draw.text(self.text, x, y, text, size, color);
    }

    pub fn label_in(&mut self, r: Rect, text: &str, size: f32, color: Color) {
        self.draw.text(self.text, r.x, r.y + (r.h - size) * 0.5, text, size, color);
    }

    pub fn label_centered(&mut self, r: Rect, text: &str, size: f32, color: Color) {
        self.draw.text_centered(self.text, r, text, size, color);
    }

    // ---- Button ----

    pub fn button(&mut self, r: Rect, label: &str) -> bool {
        let hovered = self.input.hovered(r.x, r.y, r.w, r.h);
        let clicked = self.input.clicked_in(r.x, r.y, r.w, r.h);

        let bg = if self.input.mouse_down && hovered {
            theme::ACCENT
        } else if hovered {
            theme::ACCENT_HOVER
        } else {
            theme::ACCENT_DIM
        };

        self.draw.rounded_rect(r, theme::BORDER_RADIUS, bg);
        self.draw.text_centered(self.text, r, label, theme::FONT_SIZE_NORMAL, theme::TEXT_PRIMARY);
        clicked
    }

    /// Button with custom color (for emergency stop, etc.)
    pub fn button_colored(&mut self, r: Rect, label: &str, base: Color, hover: Color) -> bool {
        let hovered = self.input.hovered(r.x, r.y, r.w, r.h);
        let clicked = self.input.clicked_in(r.x, r.y, r.w, r.h);

        let bg = if hovered { hover } else { base };
        self.draw.rounded_rect(r, theme::BORDER_RADIUS, bg);
        self.draw.text_centered(self.text, r, label, theme::FONT_SIZE_NORMAL, theme::TEXT_PRIMARY);
        clicked
    }

    /// Ghost button (no background, text only)
    pub fn button_ghost(&mut self, r: Rect, label: &str) -> bool {
        let hovered = self.input.hovered(r.x, r.y, r.w, r.h);
        let clicked = self.input.clicked_in(r.x, r.y, r.w, r.h);

        if hovered {
            self.draw.rounded_rect(r, theme::BORDER_RADIUS, theme::BG_HOVER);
        }
        self.draw.text_centered(self.text, r, label, theme::FONT_SIZE_NORMAL,
            if hovered { theme::TEXT_PRIMARY } else { theme::TEXT_SECONDARY });
        clicked
    }

    // ---- Toggle ----

    pub fn toggle(&mut self, r: Rect, label: &str, value: bool) -> Option<bool> {
        let clicked = self.input.clicked_in(r.x, r.y, r.w, r.h);

        // Track: 40x22 rounded rect
        let track_w = 40.0f32;
        let track_h = 22.0;
        let track_x = r.x;
        let track_y = r.y + (r.h - track_h) * 0.5;

        let track_color = if value { theme::ACCENT } else { theme::BG_TERTIARY };
        self.draw.rounded_rect(
            Rect::new(track_x, track_y, track_w, track_h),
            track_h * 0.5,
            track_color,
        );

        // Thumb: circle
        let thumb_r = 8.0;
        let thumb_x = if value { track_x + track_w - thumb_r * 2.0 - 3.0 } else { track_x + 3.0 };
        let thumb_y = track_y + (track_h - thumb_r * 2.0) * 0.5;
        self.draw.rounded_rect(
            Rect::new(thumb_x, thumb_y, thumb_r * 2.0, thumb_r * 2.0),
            thumb_r,
            theme::TEXT_PRIMARY,
        );

        // Label to the right
        if !label.is_empty() {
            self.draw.text(
                self.text,
                r.x + track_w + 8.0,
                r.y + (r.h - theme::FONT_SIZE_NORMAL) * 0.5,
                label,
                theme::FONT_SIZE_NORMAL,
                theme::TEXT_PRIMARY,
            );
        }

        if clicked { Some(!value) } else { None }
    }

    // ---- Slider ----

    pub fn slider(&mut self, r: Rect, label: &str, value: f32, min: f32, max: f32) -> Option<f32> {
        let hovered = self.input.hovered(r.x, r.y, r.w, r.h);

        // Label on top
        if !label.is_empty() {
            self.draw.text(self.text, r.x, r.y, label, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);
        }

        // Track
        let track_y = r.y + 20.0;
        let track_h = 4.0;
        let track_r = Rect::new(r.x, track_y, r.w, track_h);
        self.draw.rounded_rect(track_r, 2.0, theme::BG_TERTIARY);

        // Fill
        let t = ((value - min) / (max - min)).clamp(0.0, 1.0);
        let fill_w = r.w * t;
        self.draw.rounded_rect(Rect::new(r.x, track_y, fill_w, track_h), 2.0, theme::ACCENT);

        // Thumb
        let thumb_x = r.x + fill_w - 8.0;
        self.draw.rounded_rect(Rect::new(thumb_x, track_y - 6.0, 16.0, 16.0), 8.0, theme::TEXT_PRIMARY);

        // Value text
        self.draw.text(
            self.text,
            r.x + r.w + 8.0,
            track_y - 4.0,
            &format!("{:.1}", value),
            theme::FONT_SIZE_SMALL,
            theme::TEXT_SECONDARY,
        );

        // Drag
        if hovered && self.input.mouse_down {
            let new_t = ((self.input.mouse_x - r.x) / r.w).clamp(0.0, 1.0);
            let new_val = min + (max - min) * new_t;
            if (new_val - value).abs() > 0.001 {
                return Some(new_val);
            }
        }
        None
    }

    // ---- Text Input ----

    pub fn text_input(&mut self, r: Rect, id: u64, text: &str, cursor: &mut usize) -> Option<String> {
        let focused = self.input.focused_id == Some(id);
        let clicked = self.input.clicked_in(r.x, r.y, r.w, r.h);

        // Background
        let border_color = if focused { theme::BORDER_FOCUS } else { theme::BORDER };
        self.draw.rounded_rect(r, theme::BORDER_RADIUS, theme::BG_PRIMARY);
        self.draw.border(r, 1.0, border_color);

        // Text
        let text_x = r.x + 8.0;
        let text_y = r.y + (r.h - theme::FONT_SIZE_NORMAL) * 0.5;
        self.draw.text(self.text, text_x, text_y, text, theme::FONT_SIZE_NORMAL, theme::TEXT_PRIMARY);

        // Cursor blink (visible when focused)
        if focused {
            let cursor_pos = (*cursor).min(text.len());
            let cursor_x = text_x + self.text.measure(&text[..cursor_pos], theme::FONT_SIZE_NORMAL);
            self.draw.rect(Rect::new(cursor_x, text_y, 1.5, theme::FONT_SIZE_NORMAL + 2.0), theme::ACCENT);
        }

        if !focused && !clicked {
            return None;
        }

        if !focused {
            // Will be focused next frame
            return None;
        }

        // Handle typing
        let mut result = text.to_string();
        let mut changed = false;

        for &ch in &self.input.chars_typed {
            let pos = (*cursor).min(result.len());
            result.insert(pos, ch);
            *cursor += 1;
            changed = true;
        }

        if self.input.key_backspace && *cursor > 0 {
            *cursor -= 1;
            if *cursor < result.len() {
                result.remove(*cursor);
            }
            changed = true;
        }

        if self.input.key_delete && *cursor < result.len() {
            result.remove(*cursor);
            changed = true;
        }

        if self.input.key_left && *cursor > 0 {
            *cursor -= 1;
        }
        if self.input.key_right && *cursor < result.len() {
            *cursor += 1;
        }
        if self.input.key_home {
            *cursor = 0;
        }
        if self.input.key_end {
            *cursor = result.len();
        }

        if changed { Some(result) } else { None }
    }

    // ---- Tabs ----

    pub fn tabs(&mut self, r: Rect, labels: &[&str], active: usize) -> Option<usize> {
        let tab_w = r.w / labels.len() as f32;
        let mut clicked = None;

        for (i, label) in labels.iter().enumerate() {
            let tab_r = Rect::new(r.x + i as f32 * tab_w, r.y, tab_w, r.h);
            let is_active = i == active;
            let hovered = self.input.hovered(tab_r.x, tab_r.y, tab_r.w, tab_r.h);

            if is_active {
                self.draw.rect(tab_r, theme::BG_SECONDARY);
                // Active indicator line at bottom
                self.draw.rect(
                    Rect::new(tab_r.x, tab_r.y + tab_r.h - 2.0, tab_r.w, 2.0),
                    theme::ACCENT,
                );
            } else if hovered {
                self.draw.rect(tab_r, theme::BG_HOVER);
            }

            let text_color = if is_active { theme::TEXT_PRIMARY } else { theme::TEXT_SECONDARY };
            self.draw.text_centered(self.text, tab_r, label, theme::FONT_SIZE_NORMAL, text_color);

            if self.input.clicked_in(tab_r.x, tab_r.y, tab_r.w, tab_r.h) && !is_active {
                clicked = Some(i);
            }
        }

        clicked
    }

    // ---- Table Row ----

    pub fn table_row(&mut self, r: Rect, cols: &[&str], widths: &[f32], even: bool) {
        let bg = if even { theme::BG_PRIMARY } else { theme::BG_SECONDARY };
        self.draw.rect(r, bg);

        let mut x = r.x + 8.0;
        for (i, col) in cols.iter().enumerate() {
            let w = widths.get(i).copied().unwrap_or(100.0);
            self.draw.text(
                self.text,
                x,
                r.y + (r.h - theme::FONT_SIZE_SMALL) * 0.5,
                col,
                theme::FONT_SIZE_SMALL,
                if i == 0 { theme::TEXT_PRIMARY } else { theme::TEXT_SECONDARY },
            );
            x += w;
        }
    }

    /// Table header row
    pub fn table_header(&mut self, r: Rect, cols: &[&str], widths: &[f32]) {
        self.draw.rect(r, theme::BG_TERTIARY);

        let mut x = r.x + 8.0;
        for (i, col) in cols.iter().enumerate() {
            let w = widths.get(i).copied().unwrap_or(100.0);
            self.draw.text(
                self.text,
                x,
                r.y + (r.h - theme::FONT_SIZE_SMALL) * 0.5,
                col,
                theme::FONT_SIZE_SMALL,
                theme::TEXT_SECONDARY,
            );
            x += w;
        }
    }

    // ---- Progress Bar ----

    pub fn progress(&mut self, r: Rect, value: f32, color: Color) {
        self.draw.rounded_rect(r, 3.0, theme::BG_TERTIARY);
        let fill = Rect::new(r.x, r.y, r.w * value.clamp(0.0, 1.0), r.h);
        self.draw.rounded_rect(fill, 3.0, color);
    }

    /// Progress with label
    pub fn progress_labeled(&mut self, r: Rect, value: f32, color: Color, label: &str) {
        self.progress(r, value, color);
        self.draw.text_centered(self.text, r, label, theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);
    }

    // ---- Card ----

    /// Draw a panel card with title
    pub fn card(&mut self, r: Rect, title: &str) -> Rect {
        self.draw.rounded_rect(r, theme::BORDER_RADIUS, theme::BG_SECONDARY);
        self.draw.border(r, 1.0, theme::BORDER);

        let _header = Rect::new(r.x, r.y, r.w, 36.0);
        self.draw.rect(
            Rect::new(r.x, r.y + 36.0 - 1.0, r.w, 1.0),
            theme::BORDER,
        );
        self.draw.text(self.text, r.x + 12.0, r.y + 10.0, title, theme::FONT_SIZE_NORMAL, theme::TEXT_PRIMARY);

        // Return content area
        Rect::new(r.x + 8.0, r.y + 36.0 + 8.0, r.w - 16.0, r.h - 36.0 - 16.0)
    }

    // ---- Status Badge ----

    pub fn badge(&mut self, x: f32, y: f32, text: &str, color: Color) {
        let tw = self.text.measure(text, theme::FONT_SIZE_SMALL);
        let r = Rect::new(x, y, tw + 12.0, 20.0);
        self.draw.rounded_rect(r, 10.0, color.with_alpha(0.2));
        self.draw.text_centered(self.text, r, text, theme::FONT_SIZE_SMALL, color);
    }

    // ---- Separator ----

    pub fn separator(&mut self, r: Rect) {
        self.draw.rect(Rect::new(r.x, r.y + r.h * 0.5, r.w, 1.0), theme::BORDER);
    }

    // ---- Scroll Area ----

    /// Scrollable region. Returns a clipped content rect offset by scroll.
    /// Caller should use the returned rect for laying out children.
    pub fn scroll_area(&mut self, rect: Rect, content_height: f32, id: u64, scroll_offsets: &mut std::collections::HashMap<u64, f32>) -> Rect {
        let offset = scroll_offsets.entry(id).or_insert(0.0);

        // Handle mouse wheel when hovered
        if self.input.hovered(rect.x, rect.y, rect.w, rect.h) {
            *offset -= self.input.scroll_delta * 30.0;
        }

        // Clamp scroll
        let max_scroll = (content_height - rect.h).max(0.0);
        *offset = offset.clamp(0.0, max_scroll);

        // Push clip
        self.draw.push_clip(rect);

        // Return content rect shifted by scroll offset
        Rect::new(rect.x, rect.y - *offset, rect.w, content_height)
    }

    /// End a scroll area (pop clip)
    pub fn end_scroll_area(&mut self) {
        self.draw.pop_clip();
    }

    // ---- Target Selector (orchestration) ----

    /// Draws a horizontal target selector: [Selected] [All] [All FBC] [All Sonoma] [Pick...]
    /// Returns the new CommandTarget if the user clicked one.
    pub fn target_selector(&mut self, r: Rect, current: &crate::state::CommandTarget, label: &str) -> Option<crate::state::CommandTarget> {
        use crate::state::CommandTarget;

        // Label
        self.draw.text(self.text, r.x, r.y + (r.h - theme::FONT_SIZE_SMALL) * 0.5, label, theme::FONT_SIZE_SMALL, theme::TEXT_SECONDARY);

        let mut x = r.x + self.text.measure(label, theme::FONT_SIZE_SMALL) + 12.0;
        let btn_h = r.h.min(28.0);
        let btn_y = r.y + (r.h - btn_h) * 0.5;
        let mut result = None;

        let options: &[(&str, CommandTarget)] = &[
            ("Selected", CommandTarget::Selected),
            ("All", CommandTarget::All),
            ("All FBC", CommandTarget::AllFbc),
            ("All Sonoma", CommandTarget::AllSonoma),
        ];

        for (text, target) in options {
            let tw = self.text.measure(text, theme::FONT_SIZE_SMALL) + 16.0;
            let btn_r = Rect::new(x, btn_y, tw, btn_h);
            let is_active = std::mem::discriminant(current) == std::mem::discriminant(target);
            let hovered = self.input.hovered(btn_r.x, btn_r.y, btn_r.w, btn_r.h);
            let clicked = self.input.clicked_in(btn_r.x, btn_r.y, btn_r.w, btn_r.h);

            let bg = if is_active {
                theme::ACCENT_DIM
            } else if hovered {
                theme::BG_HOVER
            } else {
                theme::BG_TERTIARY
            };
            self.draw.rounded_rect(btn_r, 4.0, bg);

            let text_color = if is_active { theme::ACCENT } else { theme::TEXT_SECONDARY };
            self.draw.text_centered(self.text, btn_r, text, theme::FONT_SIZE_SMALL, text_color);

            if clicked && !is_active {
                result = Some(target.clone());
            }

            x += tw + 4.0;
        }

        result
    }

    // ---- Checkbox ----

    /// Simple checkbox: returns Some(new_value) if clicked.
    pub fn checkbox(&mut self, r: Rect, label: &str, checked: bool) -> Option<bool> {
        let clicked = self.input.clicked_in(r.x, r.y, r.w, r.h);
        let box_size = 18.0;
        let box_y = r.y + (r.h - box_size) * 0.5;

        // Box
        let box_r = Rect::new(r.x, box_y, box_size, box_size);
        self.draw.rounded_rect(box_r, 3.0, if checked { theme::ACCENT } else { theme::BG_TERTIARY });
        self.draw.border(box_r, 1.0, theme::BORDER);

        // Checkmark
        if checked {
            self.draw.text_centered(self.text, box_r, "x", theme::FONT_SIZE_SMALL, theme::TEXT_PRIMARY);
        }

        // Label
        if !label.is_empty() {
            self.draw.text(
                self.text,
                r.x + box_size + 8.0,
                r.y + (r.h - theme::FONT_SIZE_NORMAL) * 0.5,
                label, theme::FONT_SIZE_NORMAL, theme::TEXT_PRIMARY,
            );
        }

        if clicked { Some(!checked) } else { None }
    }

    // ---- Sidebar Nav Item ----

    pub fn nav_item(&mut self, r: Rect, label: &str, icon: &str, active: bool) -> bool {
        let hovered = self.input.hovered(r.x, r.y, r.w, r.h);
        let clicked = self.input.clicked_in(r.x, r.y, r.w, r.h);

        if active {
            self.draw.rounded_rect(r, theme::BORDER_RADIUS, theme::ACCENT_DIM);
            self.draw.rect(Rect::new(r.x, r.y, 3.0, r.h), theme::ACCENT);
        } else if hovered {
            self.draw.rounded_rect(r, theme::BORDER_RADIUS, theme::BG_HOVER);
        }

        let text_color = if active { theme::ACCENT } else if hovered { theme::TEXT_PRIMARY } else { theme::TEXT_SECONDARY };

        // Icon (single char)
        self.draw.text(self.text, r.x + 12.0, r.y + (r.h - theme::FONT_SIZE_NORMAL) * 0.5, icon, theme::FONT_SIZE_NORMAL, text_color);
        // Label
        self.draw.text(self.text, r.x + 36.0, r.y + (r.h - theme::FONT_SIZE_NORMAL) * 0.5, label, theme::FONT_SIZE_NORMAL, text_color);

        clicked
    }
}
