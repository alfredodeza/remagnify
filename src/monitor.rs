use crate::pool_buffer::PoolBuffer;
use crate::utils::Vector2D;
use wayland_client::protocol::wl_output::{Transform, WlOutput};

pub struct Monitor {
    pub name: String,
    pub output: WlOutput,
    pub wayland_name: u32,
    pub size: Vector2D,
    pub scale: i32,
    pub transform: Transform,
    pub ready: bool,

    // Screen capture buffer
    pub screen_buffer: Option<PoolBuffer>,
    pub screen_buffer_format: u32,
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
            transform: Transform::Normal,
            ready: false,
            screen_buffer: None,
            screen_buffer_format: 0,
            screen_flags: 0,
            layer_surface_idx: None,
        }
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
        log::debug!("Monitor {} scale: {}", self.wayland_name, scale);
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
