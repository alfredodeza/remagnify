//! Rendering pipeline for magnified screen content.
//!
//! This module handles the Cairo-based rendering of the magnifier overlay,
//! including background rendering, magnified region rendering, and outline drawing.

use crate::pool_buffer::PoolBuffer;
use crate::utils::Vector2D;
use anyhow::Result;
use cairo::{Filter, Matrix, SurfacePattern};

/// Renderer for magnified content.
///
/// Manages the zoom level and renders the magnified view using Cairo.
/// The rendering pipeline consists of three stages:
/// 1. Background: Full screen capture at reduced size
/// 2. Magnified region: Zoomed section around the pointer
/// 3. Outline: Visual frame around the magnified area
pub struct Renderer {
    /// Current zoom level (0.01 = 1%, 1.0 = 100%)
    pub zoom: f64,
}

impl Renderer {
    /// Create a new renderer with default zoom level (0.5 = 50%).
    pub fn new() -> Self {
        Self { zoom: 0.5 }
    }

    /// Set the zoom level.
    ///
    /// The zoom value is automatically clamped to the range 0.01..=1.0.
    ///
    /// # Arguments
    ///
    /// * `zoom` - Desired zoom level (will be clamped to 0.01..=1.0)
    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom.clamp(0.01, 1.0);
    }

    /// Adjust the zoom level by a delta value.
    ///
    /// The resulting zoom value is automatically clamped to the range 0.01..=1.0.
    ///
    /// # Arguments
    ///
    /// * `delta` - Amount to add to current zoom (negative to zoom out)
    #[allow(dead_code)]
    pub fn adjust_zoom(&mut self, delta: f64) {
        self.zoom = (self.zoom + delta).clamp(0.01, 1.0);
    }

    /// Render the magnified view onto the output buffer.
    ///
    /// This is the main rendering function that orchestrates the three-stage
    /// rendering pipeline:
    /// 1. Render background (full screen capture)
    /// 2. Render magnified region centered on pointer
    /// 3. Draw outline around magnified area
    ///
    /// # Arguments
    ///
    /// * `output_buffer` - Destination buffer for rendering
    /// * `screen_buffer` - Source screen capture buffer
    /// * `position` - Center position of magnifier in output coordinates
    /// * `magnifier_size` - Size of the magnified region
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Rendering succeeded
    /// * `Err` - Cairo rendering error
    pub fn render_surface(
        &self,
        output_buffer: &mut PoolBuffer,
        screen_buffer: &mut PoolBuffer,
        position: Vector2D,
        magnifier_size: Vector2D,
        force_inactive: bool,
        render_inactive: bool,
    ) -> Result<()> {
        let ctx = output_buffer.create_cairo_context()?;

        // Clear background
        ctx.save()?;
        ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
        ctx.set_operator(cairo::Operator::Source);
        ctx.rectangle(
            0.0,
            0.0,
            output_buffer.pixel_size.x,
            output_buffer.pixel_size.y,
        );
        ctx.fill()?;
        ctx.restore()?;

        if !force_inactive || render_inactive {
            // Render full screen background
            self.render_background(&ctx, screen_buffer, output_buffer)?;
        }

        if !force_inactive {
            // Render magnified region
            self.render_magnified_region(
                &ctx,
                screen_buffer,
                output_buffer,
                position,
                magnifier_size,
            )?;

            // Draw outline
            self.draw_outline(&ctx, position, magnifier_size)?;
        }

        Ok(())
    }

    fn render_background(
        &self,
        ctx: &cairo::Context,
        screen: &mut PoolBuffer,
        output: &PoolBuffer,
    ) -> Result<()> {
        let screen_surf = screen.get_cairo_surface()?;
        let pattern = SurfacePattern::create(screen_surf);
        pattern.set_filter(Filter::Bilinear);

        let scale = screen.pixel_size / output.pixel_size;
        let mut matrix = Matrix::identity();
        matrix.scale(scale.x, scale.y);
        pattern.set_matrix(matrix);

        ctx.set_source(&pattern)?;
        ctx.paint()?;

        Ok(())
    }

    fn render_magnified_region(
        &self,
        ctx: &cairo::Context,
        screen: &mut PoolBuffer,
        output: &PoolBuffer,
        position: Vector2D,
        size: Vector2D,
    ) -> Result<()> {
        let screen_surf = screen.get_cairo_surface()?;
        let pattern = SurfacePattern::create(screen_surf);
        pattern.set_filter(Filter::Nearest);

        let scale = screen.pixel_size / output.pixel_size;
        let magnifier_pos = position.floor();
        let click_pos = magnifier_pos / output.pixel_size * screen.pixel_size;

        // Calculate the transform matrix for magnification
        let mut matrix = Matrix::identity();
        matrix.translate(click_pos.x, click_pos.y);
        matrix.scale(self.zoom, self.zoom);
        matrix.translate(-click_pos.x / scale.x, -click_pos.y / scale.y);
        pattern.set_matrix(matrix);

        ctx.set_source(&pattern)?;

        // Clip to magnifier region
        ctx.save()?;
        ctx.rectangle(
            magnifier_pos.x - size.x / 2.0,
            magnifier_pos.y - size.y / 2.0,
            size.x,
            size.y,
        );
        ctx.clip();
        ctx.paint()?;
        ctx.restore()?;

        Ok(())
    }

    fn draw_outline(
        &self,
        ctx: &cairo::Context,
        position: Vector2D,
        size: Vector2D,
    ) -> Result<()> {
        ctx.rectangle(
            position.x - size.x / 2.0,
            position.y - size.y / 2.0,
            size.x,
            size.y,
        );
        ctx.set_source_rgba(150.0 / 255.0, 150.0 / 255.0, 150.0 / 255.0, 1.0);
        ctx.set_line_width(2.0);
        ctx.stroke()?;

        Ok(())
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_default_zoom() {
        let renderer = Renderer::new();
        assert_eq!(renderer.zoom, 0.5);
    }

    #[test]
    fn test_set_zoom_clamping() {
        let mut renderer = Renderer::new();

        // Test valid zoom
        renderer.set_zoom(0.75);
        assert_eq!(renderer.zoom, 0.75);

        // Test zoom too low - should clamp to 0.01
        renderer.set_zoom(-0.5);
        assert_eq!(renderer.zoom, 0.01);

        // Test zoom too high - should clamp to 1.0
        renderer.set_zoom(5.0);
        assert_eq!(renderer.zoom, 1.0);

        // Test edge cases
        renderer.set_zoom(0.01);
        assert_eq!(renderer.zoom, 0.01);

        renderer.set_zoom(1.0);
        assert_eq!(renderer.zoom, 1.0);
    }

    #[test]
    fn test_adjust_zoom_clamping() {
        let mut renderer = Renderer::new();
        assert_eq!(renderer.zoom, 0.5);

        // Test positive adjustment
        renderer.adjust_zoom(0.2);
        assert!((renderer.zoom - 0.7).abs() < 1e-10);

        // Test negative adjustment
        renderer.adjust_zoom(-0.3);
        assert!((renderer.zoom - 0.4).abs() < 1e-10);

        // Test adjustment that would exceed max - should clamp
        renderer.adjust_zoom(1.0);
        assert_eq!(renderer.zoom, 1.0);

        // Test adjustment that would go below min - should clamp
        renderer.adjust_zoom(-2.0);
        assert_eq!(renderer.zoom, 0.01);
    }
}
