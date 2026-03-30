/// Input state tracking — mouse position, clicks, keyboard buffer

use winit::event::{ElementState, KeyEvent, MouseButton, WindowEvent};
use winit::keyboard::{Key, NamedKey};

pub struct InputState {
    pub mouse_x: f32,
    pub mouse_y: f32,
    pub mouse_down: bool,
    pub mouse_clicked: bool,       // true for one frame on press
    pub mouse_released: bool,      // true for one frame on release
    pub mouse_prev_down: bool,
    pub scroll_delta: f32,

    // Keyboard
    pub chars_typed: Vec<char>,
    pub key_backspace: bool,
    pub key_delete: bool,
    pub key_enter: bool,
    pub key_escape: bool,
    pub key_tab: bool,
    pub key_left: bool,
    pub key_right: bool,
    pub key_home: bool,
    pub key_end: bool,
    pub key_ctrl: bool,
    pub key_shift: bool,

    // Focus
    pub focused_id: Option<u64>,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            mouse_x: 0.0,
            mouse_y: 0.0,
            mouse_down: false,
            mouse_clicked: false,
            mouse_released: false,
            mouse_prev_down: false,
            scroll_delta: 0.0,
            chars_typed: Vec::new(),
            key_backspace: false,
            key_delete: false,
            key_enter: false,
            key_escape: false,
            key_tab: false,
            key_left: false,
            key_right: false,
            key_home: false,
            key_end: false,
            key_ctrl: false,
            key_shift: false,
            focused_id: None,
        }
    }

    /// Call at start of frame to reset per-frame flags
    pub fn begin_frame(&mut self) {
        self.mouse_clicked = self.mouse_down && !self.mouse_prev_down;
        self.mouse_released = !self.mouse_down && self.mouse_prev_down;
        self.mouse_prev_down = self.mouse_down;
        self.scroll_delta = 0.0;
        self.chars_typed.clear();
        self.key_backspace = false;
        self.key_delete = false;
        self.key_enter = false;
        self.key_escape = false;
        self.key_tab = false;
        self.key_left = false;
        self.key_right = false;
        self.key_home = false;
        self.key_end = false;
    }

    pub fn handle_event(&mut self, event: &WindowEvent) {
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_x = position.x as f32;
                self.mouse_y = position.y as f32;
            }
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => {
                self.mouse_down = *state == ElementState::Pressed;
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.scroll_delta = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => *y * 40.0,
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y as f32,
                };
            }
            WindowEvent::ModifiersChanged(mods) => {
                let state = mods.state();
                self.key_ctrl = state.control_key();
                self.key_shift = state.shift_key();
            }
            WindowEvent::KeyboardInput { event: KeyEvent { logical_key, state: ElementState::Pressed, .. }, .. } => {
                match logical_key {
                    Key::Named(NamedKey::Backspace) => self.key_backspace = true,
                    Key::Named(NamedKey::Delete) => self.key_delete = true,
                    Key::Named(NamedKey::Enter) => self.key_enter = true,
                    Key::Named(NamedKey::Escape) => self.key_escape = true,
                    Key::Named(NamedKey::Tab) => self.key_tab = true,
                    Key::Named(NamedKey::ArrowLeft) => self.key_left = true,
                    Key::Named(NamedKey::ArrowRight) => self.key_right = true,
                    Key::Named(NamedKey::Home) => self.key_home = true,
                    Key::Named(NamedKey::End) => self.key_end = true,
                    Key::Character(c) => {
                        for ch in c.chars() {
                            if !ch.is_control() {
                                self.chars_typed.push(ch);
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    /// Check if mouse is hovering a rect
    pub fn hovered(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse_x >= x && self.mouse_x < x + w
            && self.mouse_y >= y && self.mouse_y < y + h
    }

    /// Check if mouse clicked inside a rect
    pub fn clicked_in(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        self.mouse_clicked && self.hovered(x, y, w, h)
    }
}
