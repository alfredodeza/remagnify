use crate::pool_buffer::PoolBuffer;
use crate::utils::Vector2D;
use anyhow::Result;
use cairo::{Filter, Matrix, SurfacePattern};

pub struct Renderer {
    pub zoom: f64,
}

impl Renderer {
    pub fn new() -> Self {
        Self { zoom: 0.5 }
    }

    pub fn set_zoom(&mut self, zoom: f64) {
        self.zoom = zoom.clamp(0.01, 1.0);
    }

    pub fn adjust_zoom(&mut self, delta: f64) {
        self.zoom = (self.zoom + delta).clamp(0.01, 1.0);
    }

    /// Render the magnified view onto the output buffer
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
