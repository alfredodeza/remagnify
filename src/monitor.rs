use crate::pool_buffer::PoolBuffer;
use crate::utils::Vector2D;
use wayland_client::protocol::wl_output::{Transform, WlOutput};

pub struct Monitor {
    pub name: String,
    pub output: WlOutput,
    pub wayland_name: u32,
    pub size: Vector2D,
    pub scale: i32,
    pub fractional_scale: f64, // Actual fractional scale (e.g., 1.5)
    #[allow(dead_code)]
    pub transform: Transform,
    pub ready: bool,

    // Screen capture buffer
    pub screen_buffer: Option<PoolBuffer>,
    pub screen_buffer_format: u32,
    #[allow(dead_code)]
    pub screen_flags: u32,

    // Layer surface index
    pub layer_surface_idx: Option<usize>,
}

impl Monitor {
    pub fn new(output: WlOutput, wayland_name: u32) -> Self {
        Self {
            name: String::new(),
            output,
            wayland_name,
            size: Vector2D::default(),
            scale: 1,
            fractional_scale: 1.0,
            transform: Transform::Normal,
            ready: false,
            screen_buffer: None,
            screen_buffer_format: 0,
            screen_flags: 0,
            layer_surface_idx: None,
        }
    }

    /// Get the logical size of the monitor based on physical size and fractional scale
    pub fn get_logical_size(&self) -> Vector2D {
        Vector2D::new(
            self.size.x / self.fractional_scale,
            self.size.y / self.fractional_scale,
        )
    }

    pub fn set_geometry(&mut self, x: i32, y: i32, width: i32, height: i32) {
        log::debug!(
            "Monitor {} geometry: {}x{} at ({}, {})",
            self.wayland_name,
            width,
            height,
            x,
            y
        );
    }

    pub fn set_mode(&mut self, width: i32, height: i32, refresh: i32) {
        self.size = Vector2D::new(width as f64, height as f64);
        log::debug!(
            "Monitor {} mode: {}x{} @ {}Hz",
            self.wayland_name,
            width,
            height,
            refresh / 1000
        );
    }

    pub fn set_scale(&mut self, scale: i32) {
        self.scale = scale;
        self.fractional_scale = scale as f64;
        log::debug!(
            "Monitor {} scale: {} (fractional: {})",
            self.wayland_name,
            scale,
            self.fractional_scale
        );
    }

    /// Override the fractional scale value (e.g., from CLI --scale option).
    /// Use this when the integer scale from wl_output doesn't match the actual scaling.
    pub fn set_fractional_scale(&mut self, fractional_scale: f64) {
        self.fractional_scale = fractional_scale;
        if (self.fractional_scale - self.scale as f64).abs() > 0.01 {
            log::info!(
                "Monitor {} using fractional scale: {} (wl_output reported: {})",
                self.wayland_name,
                self.fractional_scale,
                self.scale
            );
        }
    }

    pub fn set_name(&mut self, name: String) {
        self.name = name;
        log::debug!("Monitor {} name: {}", self.wayland_name, self.name);
    }

    pub fn set_done(&mut self) {
        self.ready = true;
        log::info!(
            "Monitor {} ready: {} ({}x{} @ scale {})",
            self.wayland_name,
            self.name,
            self.size.x,
            self.size.y,
            self.scale
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_logical_size_fractional_scale() {
        // Test logical size calculation with fractional scaling (1.5x)
        let size = Vector2D::new(1920.0, 1200.0);
        let scale = 1.5;

        let logical_x = size.x / scale;
        let logical_y = size.y / scale;

        assert!((logical_x - 1280.0).abs() < 0.01);
        assert!((logical_y - 800.0).abs() < 0.01);
    }

    #[test]
    fn test_get_logical_size_integer_scale() {
        // Test logical size calculation with integer scaling (2x)
        let size = Vector2D::new(1920.0, 1080.0);
        let scale = 2.0;

        let logical_x = size.x / scale;
        let logical_y = size.y / scale;

        assert_eq!(logical_x, 960.0);
        assert_eq!(logical_y, 540.0);
    }

    #[test]
    fn test_get_logical_size_no_scale() {
        // Test logical size calculation with no scaling (1x)
        let size = Vector2D::new(1920.0, 1080.0);
        let scale = 1.0;

        let logical_x = size.x / scale;
        let logical_y = size.y / scale;

        assert_eq!(logical_x, 1920.0);
        assert_eq!(logical_y, 1080.0);
    }

    #[test]
    fn test_fractional_scale_calculations() {
        // Test various fractional scales
        let test_cases = vec![
            (1920.0, 1200.0, 1.25, 1536.0, 960.0),
            (2560.0, 1440.0, 1.5, 1706.67, 960.0),
            (3840.0, 2160.0, 2.0, 1920.0, 1080.0),
        ];

        for (width, height, scale, expected_w, expected_h) in test_cases {
            let size = Vector2D::new(width, height);
            let logical_w = size.x / scale;
            let logical_h = size.y / scale;

            assert!(
                (logical_w - expected_w).abs() < 1.0,
                "Width mismatch for {}x{} at {}x scale",
                width,
                height,
                scale
            );
            assert!(
                (logical_h - expected_h).abs() < 1.0,
                "Height mismatch for {}x{} at {}x scale",
                width,
                height,
                scale
            );
        }
    }
}
