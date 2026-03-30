/// Quad batcher — collects colored rects, textured quads, and text glyphs,
/// then draws them all in one vertex buffer per frame.
/// Supports scissor rect clipping via push_clip/pop_clip.

use crate::gpu::{Gpu, Vertex2D};
use crate::text::TextRenderer;
use crate::theme::Color;
use crate::layout::Rect;

/// A draw command with its clip state
struct DrawBatch {
    start: u32,
    count: u32,
    clip: Option<Rect>,
}

pub struct DrawList {
    vertices: Vec<Vertex2D>,
    /// Scissor stack for clipping
    clip_stack: Vec<Rect>,
    /// Batches separated by clip changes
    batches: Vec<DrawBatch>,
    /// Current batch start vertex
    batch_start: u32,
    /// Current clip rect
    current_clip: Option<Rect>,
}

impl DrawList {
    pub fn new() -> Self {
        Self {
            vertices: Vec::with_capacity(16384),
            clip_stack: Vec::new(),
            batches: Vec::new(),
            batch_start: 0,
            current_clip: None,
        }
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.clip_stack.clear();
        self.batches.clear();
        self.batch_start = 0;
        self.current_clip = None;
    }

    /// Push a clipping rectangle
    pub fn push_clip(&mut self, rect: Rect) {
        self.flush_batch();
        self.clip_stack.push(rect);
        self.current_clip = Some(rect);
    }

    pub fn pop_clip(&mut self) {
        self.flush_batch();
        self.clip_stack.pop();
        self.current_clip = self.clip_stack.last().copied();
    }

    /// Flush current vertices into a batch
    fn flush_batch(&mut self) {
        let end = self.vertices.len() as u32;
        if end > self.batch_start {
            self.batches.push(DrawBatch {
                start: self.batch_start,
                count: end - self.batch_start,
                clip: self.current_clip,
            });
        }
        self.batch_start = end;
    }

    /// Draw a solid colored rectangle
    pub fn rect(&mut self, r: Rect, color: Color) {
        let c = color.0;
        let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.w, r.y + r.h);
        // Two triangles, no UV (0,0)
        self.vertices.extend_from_slice(&[
            Vertex2D { pos: [x0, y0], uv: [0.0, 0.0], color: c },
            Vertex2D { pos: [x1, y0], uv: [0.0, 0.0], color: c },
            Vertex2D { pos: [x0, y1], uv: [0.0, 0.0], color: c },
            Vertex2D { pos: [x1, y0], uv: [0.0, 0.0], color: c },
            Vertex2D { pos: [x1, y1], uv: [0.0, 0.0], color: c },
            Vertex2D { pos: [x0, y1], uv: [0.0, 0.0], color: c },
        ]);
    }

    /// Draw a rectangle with rounded corners (approximated with extra triangles)
    pub fn rounded_rect(&mut self, r: Rect, radius: f32, color: Color) {
        if radius <= 1.0 {
            self.rect(r, color);
            return;
        }
        let rad = radius.min(r.w * 0.5).min(r.h * 0.5);

        // Center cross (horizontal)
        self.rect(Rect::new(r.x + rad, r.y, r.w - rad * 2.0, r.h), color);
        // Left strip
        self.rect(Rect::new(r.x, r.y + rad, rad, r.h - rad * 2.0), color);
        // Right strip
        self.rect(Rect::new(r.x + r.w - rad, r.y + rad, rad, r.h - rad * 2.0), color);

        // Corners (6 segments each, approximate circle)
        let segments = 6;
        let corners = [
            (r.x + rad, r.y + rad),             // top-left
            (r.x + r.w - rad, r.y + rad),       // top-right
            (r.x + r.w - rad, r.y + r.h - rad), // bottom-right
            (r.x + rad, r.y + r.h - rad),       // bottom-left
        ];
        let start_angles = [
            std::f32::consts::PI,
            std::f32::consts::PI * 1.5,
            0.0,
            std::f32::consts::FRAC_PI_2,
        ];

        for (i, &(cx, cy)) in corners.iter().enumerate() {
            let start = start_angles[i];
            for s in 0..segments {
                let a0 = start + (s as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;
                let a1 = start + ((s + 1) as f32 / segments as f32) * std::f32::consts::FRAC_PI_2;
                let c = color.0;
                self.vertices.extend_from_slice(&[
                    Vertex2D { pos: [cx, cy], uv: [0.0, 0.0], color: c },
                    Vertex2D { pos: [cx + a0.cos() * rad, cy + a0.sin() * rad], uv: [0.0, 0.0], color: c },
                    Vertex2D { pos: [cx + a1.cos() * rad, cy + a1.sin() * rad], uv: [0.0, 0.0], color: c },
                ]);
            }
        }
    }

    /// Draw a 1px border around a rect
    pub fn border(&mut self, r: Rect, thickness: f32, color: Color) {
        // Top
        self.rect(Rect::new(r.x, r.y, r.w, thickness), color);
        // Bottom
        self.rect(Rect::new(r.x, r.y + r.h - thickness, r.w, thickness), color);
        // Left
        self.rect(Rect::new(r.x, r.y, thickness, r.h), color);
        // Right
        self.rect(Rect::new(r.x + r.w - thickness, r.y, thickness, r.h), color);
    }

    /// Draw text at a position. Uses negative UV.x to signal alpha-from-texture mode.
    pub fn text(&mut self, text_renderer: &TextRenderer, x: f32, y: f32, text: &str, size: f32, color: Color) {
        let mut cx = x;
        let c = color.0;

        for ch in text.chars() {
            if let Some(gm) = text_renderer.glyph(ch, size) {
                if gm.width > 0.0 && gm.height > 0.0 {
                    let gx = cx + gm.offset_x;
                    // fontdue offset_y is from baseline going up; we need screen coords going down
                    let gy = y + size - gm.offset_y - gm.height;

                    let x0 = gx;
                    let y0 = gy;
                    let x1 = gx + gm.width;
                    let y1 = gy + gm.height;

                    // Negative UV.x signals: use texture alpha, vertex color
                    let u0 = -gm.uv_x;
                    let v0 = gm.uv_y;
                    let u1 = -(gm.uv_x + gm.uv_w);
                    let v1 = gm.uv_y + gm.uv_h;

                    self.vertices.extend_from_slice(&[
                        Vertex2D { pos: [x0, y0], uv: [u0, v0], color: c },
                        Vertex2D { pos: [x1, y0], uv: [u1, v0], color: c },
                        Vertex2D { pos: [x0, y1], uv: [u0, v1], color: c },
                        Vertex2D { pos: [x1, y0], uv: [u1, v0], color: c },
                        Vertex2D { pos: [x1, y1], uv: [u1, v1], color: c },
                        Vertex2D { pos: [x0, y1], uv: [u0, v1], color: c },
                    ]);
                }
                cx += gm.advance;
            }
        }
    }

    /// Draw text centered in a rect
    pub fn text_centered(&mut self, text_renderer: &TextRenderer, r: Rect, text: &str, size: f32, color: Color) {
        let tw = text_renderer.measure(text, size);
        let x = r.x + (r.w - tw) * 0.5;
        let y = r.y + (r.h - size) * 0.5;
        self.text(text_renderer, x, y, text, size, color);
    }

    /// Total vertex count
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Render the draw list with scissor rect support
    pub fn render(&mut self, gpu: &Gpu, text: &TextRenderer, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if self.vertices.is_empty() {
            return;
        }

        // Flush any remaining vertices into the final batch
        self.flush_batch();

        // Update screen size uniform
        gpu.queue.write_buffer(
            &text.screen_uniform,
            0,
            bytemuck::bytes_of(&[gpu.width as f32, gpu.height as f32]),
        );

        // Create vertex buffer
        let vertex_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vb-2d"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("pass-2d"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load, // Don't clear — 3D may have drawn first
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        pass.set_pipeline(&gpu.pipeline_2d);
        pass.set_bind_group(0, &text.bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));

        if self.batches.is_empty() {
            // No batches (shouldn't happen after flush, but safety)
            pass.draw(0..self.vertices.len() as u32, 0..1);
        } else {
            for batch in &self.batches {
                if batch.count == 0 { continue; }

                match batch.clip {
                    Some(clip) => {
                        let x = (clip.x as u32).min(gpu.width);
                        let y = (clip.y as u32).min(gpu.height);
                        let w = (clip.w as u32).min(gpu.width - x);
                        let h = (clip.h as u32).min(gpu.height - y);
                        if w > 0 && h > 0 {
                            pass.set_scissor_rect(x, y, w, h);
                        }
                    }
                    None => {
                        pass.set_scissor_rect(0, 0, gpu.width, gpu.height);
                    }
                }

                pass.draw(batch.start..(batch.start + batch.count), 0..1);
            }
        }
    }
}

use wgpu::util::DeviceExt;
