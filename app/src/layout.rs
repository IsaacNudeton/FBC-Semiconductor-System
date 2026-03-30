/// Rect-based layout engine. No CSS, no flexbox spec — just rect subdivision.

#[derive(Clone, Copy, Debug)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0, w: 0.0, h: 0.0 };

    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn pos(&self) -> [f32; 2] {
        [self.x, self.y]
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn center(&self) -> [f32; 2] {
        [self.x + self.w * 0.5, self.y + self.h * 0.5]
    }

    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px < self.x + self.w && py >= self.y && py < self.y + self.h
    }

    pub fn padded(&self, p: f32) -> Self {
        Self {
            x: self.x + p,
            y: self.y + p,
            w: (self.w - p * 2.0).max(0.0),
            h: (self.h - p * 2.0).max(0.0),
        }
    }

    pub fn shrink_left(&self, amount: f32) -> Self {
        Self {
            x: self.x + amount,
            y: self.y,
            w: (self.w - amount).max(0.0),
            h: self.h,
        }
    }

    pub fn take_left(&self, amount: f32) -> Self {
        Self {
            x: self.x,
            y: self.y,
            w: amount.min(self.w),
            h: self.h,
        }
    }

    pub fn take_right(&self, amount: f32) -> Self {
        let w = amount.min(self.w);
        Self {
            x: self.x + self.w - w,
            y: self.y,
            w,
            h: self.h,
        }
    }

    pub fn take_top(&self, amount: f32) -> Self {
        Self {
            x: self.x,
            y: self.y,
            w: self.w,
            h: amount.min(self.h),
        }
    }

    pub fn take_bottom(&self, amount: f32) -> Self {
        let h = amount.min(self.h);
        Self {
            x: self.x,
            y: self.y + self.h - h,
            w: self.w,
            h,
        }
    }

    /// Place a rect of given size at right edge, vertically centered
    pub fn right_align(&self, w: f32, h: f32) -> Self {
        Self {
            x: self.right() - w,
            y: self.y + (self.h - h) * 0.5,
            w,
            h,
        }
    }

    /// Grid subdivision: cols x rows with gap
    pub fn grid(&self, cols: usize, rows: usize, gap: f32) -> Vec<Rect> {
        let cw = (self.w - gap * (cols as f32 - 1.0)) / cols as f32;
        let rh = (self.h - gap * (rows as f32 - 1.0)) / rows as f32;
        let mut cells = Vec::with_capacity(cols * rows);
        for r in 0..rows {
            for c in 0..cols {
                cells.push(Rect {
                    x: self.x + c as f32 * (cw + gap),
                    y: self.y + r as f32 * (rh + gap),
                    w: cw,
                    h: rh,
                });
            }
        }
        cells
    }
}

/// Column layout — splits a rect top-to-bottom
pub struct Column {
    rect: Rect,
    cursor: f32,
    gap: f32,
}

impl Column {
    pub fn new(rect: Rect) -> Self {
        Self { rect, cursor: rect.y, gap: 0.0 }
    }

    pub fn with_gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Take the next `height` pixels from top
    pub fn next(&mut self, height: f32) -> Rect {
        let r = Rect {
            x: self.rect.x,
            y: self.cursor,
            w: self.rect.w,
            h: height.min(self.remaining_height()),
        };
        self.cursor += r.h + self.gap;
        r
    }

    /// All remaining space
    pub fn remaining(&self) -> Rect {
        Rect {
            x: self.rect.x,
            y: self.cursor,
            w: self.rect.w,
            h: (self.rect.bottom() - self.cursor).max(0.0),
        }
    }

    pub fn remaining_height(&self) -> f32 {
        (self.rect.bottom() - self.cursor).max(0.0)
    }

    /// Split remaining into N proportional parts
    pub fn split(&self, ratios: &[f32]) -> Vec<Rect> {
        let total: f32 = ratios.iter().sum();
        let avail = self.remaining_height();
        let mut y = self.cursor;
        ratios.iter().map(|r| {
            let h = avail * r / total;
            let rect = Rect { x: self.rect.x, y, w: self.rect.w, h };
            y += h;
            rect
        }).collect()
    }
}

/// Row layout — splits a rect left-to-right
pub struct Row {
    rect: Rect,
    cursor: f32,
    gap: f32,
}

impl Row {
    pub fn new(rect: Rect) -> Self {
        Self { rect, cursor: rect.x, gap: 0.0 }
    }

    pub fn with_gap(mut self, gap: f32) -> Self {
        self.gap = gap;
        self
    }

    /// Take the next `width` pixels from left
    pub fn next(&mut self, width: f32) -> Rect {
        let r = Rect {
            x: self.cursor,
            y: self.rect.y,
            w: width.min(self.remaining_width()),
            h: self.rect.h,
        };
        self.cursor += r.w + self.gap;
        r
    }

    /// All remaining space
    pub fn remaining(&self) -> Rect {
        Rect {
            x: self.cursor,
            y: self.rect.y,
            w: (self.rect.right() - self.cursor).max(0.0),
            h: self.rect.h,
        }
    }

    pub fn remaining_width(&self) -> f32 {
        (self.rect.right() - self.cursor).max(0.0)
    }

    /// Split remaining into N proportional parts
    pub fn split(&self, ratios: &[f32]) -> Vec<Rect> {
        let total: f32 = ratios.iter().sum();
        let avail = self.remaining_width();
        let mut x = self.cursor;
        ratios.iter().map(|r| {
            let w = avail * r / total;
            let rect = Rect { x, y: self.rect.y, w, h: self.rect.h };
            x += w;
            rect
        }).collect()
    }
}
