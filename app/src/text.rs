/// Glyph atlas from embedded font — rasterize all ASCII at startup

use std::collections::HashMap;
use crate::gpu::Gpu;

pub const ATLAS_SIZE: u32 = 1024;

/// Metrics for a single rendered glyph
#[derive(Clone, Copy)]
pub struct GlyphMetrics {
    pub uv_x: f32,
    pub uv_y: f32,
    pub uv_w: f32,
    pub uv_h: f32,
    pub width: f32,
    pub height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub advance: f32,
}

/// Pre-rendered glyph atlas with metrics lookup
pub struct TextRenderer {
    pub bind_group: wgpu::BindGroup,
    pub screen_uniform: wgpu::Buffer,
    glyphs: HashMap<(char, u32), GlyphMetrics>,  // (char, size_key) -> metrics
    line_heights: HashMap<u32, f32>,
}

impl TextRenderer {
    pub fn new(gpu: &Gpu, font_data: &[u8]) -> Self {
        let font = fontdue::Font::from_bytes(font_data, fontdue::FontSettings::default())
            .expect("parse font");

        // Rasterize ASCII 32-126 at multiple sizes
        let sizes = [12.0f32, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0];
        let mut atlas_data = vec![0u8; (ATLAS_SIZE * ATLAS_SIZE) as usize];
        let mut glyphs = HashMap::new();
        let mut line_heights = HashMap::new();

        let mut cursor_x: u32 = 1;
        let mut cursor_y: u32 = 1;
        let mut row_height: u32 = 0;

        for &size in &sizes {
            let size_key = (size * 10.0) as u32;
            let metrics = font.horizontal_line_metrics(size).unwrap();
            line_heights.insert(size_key, metrics.new_line_size);

            for ch in ' '..='~' {
                let (fm, bitmap) = font.rasterize(ch, size);

                // Advance to next row if needed
                if cursor_x + fm.width as u32 + 1 >= ATLAS_SIZE {
                    cursor_x = 1;
                    cursor_y += row_height + 1;
                    row_height = 0;
                }

                if cursor_y + fm.height as u32 + 1 >= ATLAS_SIZE {
                    break; // atlas full
                }

                // Copy bitmap into atlas
                for y in 0..fm.height {
                    for x in 0..fm.width {
                        let src = y * fm.width + x;
                        let dst = (cursor_y + y as u32) * ATLAS_SIZE + cursor_x + x as u32;
                        atlas_data[dst as usize] = bitmap[src];
                    }
                }

                let gm = GlyphMetrics {
                    uv_x: cursor_x as f32 / ATLAS_SIZE as f32,
                    uv_y: cursor_y as f32 / ATLAS_SIZE as f32,
                    uv_w: fm.width as f32 / ATLAS_SIZE as f32,
                    uv_h: fm.height as f32 / ATLAS_SIZE as f32,
                    width: fm.width as f32,
                    height: fm.height as f32,
                    offset_x: fm.xmin as f32,
                    offset_y: fm.ymin as f32,
                    advance: fm.advance_width,
                };
                glyphs.insert((ch, size_key), gm);

                cursor_x += fm.width as u32 + 1;
                row_height = row_height.max(fm.height as u32);
            }

            // New row for next size
            cursor_x = 1;
            cursor_y += row_height + 1;
            row_height = 0;
        }

        // Upload atlas to GPU as R8Unorm texture
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph-atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        gpu.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(ATLAS_SIZE),
                rows_per_image: Some(ATLAS_SIZE),
            },
            wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glyph-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Screen size uniform buffer
        let screen_uniform = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("screen-uniform"),
            size: 8, // vec2<f32>
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg-2d"),
            layout: &gpu.bind_group_layout_2d,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: screen_uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        Self {
            bind_group,
            screen_uniform,
            glyphs,
            line_heights,
        }
    }

    /// Get glyph metrics, snapping to nearest available size
    pub fn glyph(&self, ch: char, size: f32) -> Option<&GlyphMetrics> {
        let size_key = Self::snap_size(size);
        self.glyphs.get(&(ch, size_key))
            .or_else(|| self.glyphs.get(&('?', size_key)))
    }

    /// Measure text width at given size
    pub fn measure(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .filter_map(|ch| self.glyph(ch, size))
            .map(|g| g.advance)
            .sum()
    }

    /// Line height for a given size
    pub fn line_height(&self, size: f32) -> f32 {
        let key = Self::snap_size(size);
        self.line_heights.get(&key).copied().unwrap_or(size * 1.2)
    }

    fn snap_size(size: f32) -> u32 {
        let sizes = [12.0f32, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0];
        let mut best = sizes[0];
        let mut best_dist = (size - best).abs();
        for &s in &sizes[1..] {
            let d = (size - s).abs();
            if d < best_dist {
                best = s;
                best_dist = d;
            }
        }
        (best * 10.0) as u32
    }
}
