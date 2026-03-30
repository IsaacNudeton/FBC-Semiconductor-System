/// Maximus Sand palette — matches the existing CSS variables
/// All colors as [r, g, b, a] in 0.0..1.0 range

#[derive(Clone, Copy)]
pub struct Color(pub [f32; 4]);

impl Color {
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self([r, g, b, a])
    }

    pub const fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self([r, g, b, 1.0])
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self([self.0[0], self.0[1], self.0[2], a])
    }

    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self([
            self.0[0] + (other.0[0] - self.0[0]) * t,
            self.0[1] + (other.0[1] - self.0[1]) * t,
            self.0[2] + (other.0[2] - self.0[2]) * t,
            self.0[3] + (other.0[3] - self.0[3]) * t,
        ])
    }
}

impl From<Color> for [f32; 4] {
    fn from(c: Color) -> [f32; 4] {
        c.0
    }
}

// Convert hex to float: 0x0d = 13 → 13/255 = 0.051
const fn h(v: u8) -> f32 {
    v as f32 / 255.0
}

// Backgrounds
pub const BG_PRIMARY: Color = Color::rgb(h(0x0d), h(0x11), h(0x17));     // #0d1117
pub const BG_SECONDARY: Color = Color::rgb(h(0x16), h(0x1b), h(0x22));   // #161b22
pub const BG_TERTIARY: Color = Color::rgb(h(0x21), h(0x26), h(0x2d));    // #21262d
pub const BG_HOVER: Color = Color::rgb(h(0x30), h(0x36), h(0x3d));       // #30363d

// Accent
pub const ACCENT: Color = Color::rgb(h(0x44), h(0x88), h(0xff));         // #4488ff
pub const ACCENT_HOVER: Color = Color::rgb(h(0x55), h(0x99), h(0xff));   // #5599ff
pub const ACCENT_DIM: Color = Color::rgb(h(0x22), h(0x44), h(0x88));     // #224488

// Text
pub const TEXT_PRIMARY: Color = Color::rgb(h(0xe6), h(0xed), h(0xf3));   // #e6edf3
pub const TEXT_SECONDARY: Color = Color::rgb(h(0x8b), h(0x94), h(0x9e)); // #8b949e
pub const TEXT_DISABLED: Color = Color::rgb(h(0x48), h(0x4f), h(0x58));  // #484f58

// Status
pub const SUCCESS: Color = Color::rgb(h(0x00), h(0xd2), h(0x6a));       // #00d26a
pub const WARNING: Color = Color::rgb(h(0xff), h(0xc1), h(0x07));       // #ffc107
pub const ERROR: Color = Color::rgb(h(0xff), h(0x44), h(0x44));         // #ff4444
pub const IDLE: Color = Color::rgb(h(0x66), h(0x66), h(0x66));          // #666666

// Board state LEDs
pub const LED_RUNNING: Color = Color::rgb(h(0x00), h(0xff), h(0x66));   // bright green
pub const LED_DONE: Color = Color::rgb(h(0x44), h(0x88), h(0xff));      // blue
pub const LED_ERROR: Color = Color::rgb(h(0xff), h(0x22), h(0x22));     // bright red
pub const LED_IDLE: Color = Color::rgb(h(0x44), h(0x66), h(0x44));      // dim green
pub const LED_DISCONNECTED: Color = Color::rgb(h(0x44), h(0x44), h(0x44)); // gray

// Borders
pub const BORDER: Color = Color::rgb(h(0x30), h(0x36), h(0x3d));        // #30363d
pub const BORDER_FOCUS: Color = ACCENT;

// Emergency
pub const EMERGENCY: Color = Color::rgb(h(0xff), h(0x00), h(0x00));     // pure red

// Sizes
pub const SIDEBAR_WIDTH: f32 = 240.0;
pub const SIDEBAR_COLLAPSED_WIDTH: f32 = 56.0;
pub const HEADER_HEIGHT: f32 = 48.0;
pub const ROW_HEIGHT: f32 = 36.0;
pub const PADDING: f32 = 12.0;
pub const BORDER_RADIUS: f32 = 6.0;
pub const FONT_SIZE_SMALL: f32 = 12.0;
pub const FONT_SIZE_NORMAL: f32 = 14.0;
pub const FONT_SIZE_LARGE: f32 = 18.0;
pub const FONT_SIZE_TITLE: f32 = 24.0;
