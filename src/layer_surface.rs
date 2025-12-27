use crate::pool_buffer::PoolBuffer;
use crate::utils::Vector2D;
use wayland_client::protocol::{wl_callback::WlCallback, wl_surface::WlSurface};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

pub struct LayerSurface {
    pub monitor_idx: usize,
    pub surface: WlSurface,
    pub layer_surface: Option<ZwlrLayerSurfaceV1>,
    pub fractional_scale_value: f64,
    pub configured: bool,
    pub ack_serial: u32,
    pub working: bool,

    // Double buffering
    pub last_buffer: usize,
    pub buffers: [Option<PoolBuffer>; 2],

    pub dirty: bool,
    pub rendered: bool,
    pub frame_callback: Option<WlCallback>,

    // Monitor size
    pub monitor_size: Vector2D,
    pub monitor_scale: i32,
}

impl LayerSurface {
    pub fn new(monitor_idx: usize, surface: WlSurface, monitor_size: Vector2D, monitor_scale: i32) -> Self {
        Self {
            monitor_idx,
            surface,
            layer_surface: None,
            fractional_scale_value: 1.0,
            configured: false,
            ack_serial: 0,
            working: false,
            last_buffer: 0,
            buffers: [None, None],
            dirty: false,
            rendered: false,
            frame_callback: None,
            monitor_size,
            monitor_scale,
        }
    }

    pub fn get_available_buffer(&mut self) -> Option<&mut PoolBuffer> {
        // With double buffering, always return the buffer that wasn't last used
        // The other buffer is attached to the surface
        let next_buffer_idx = if self.last_buffer == 0 { 1 } else { 0 };
        self.buffers[next_buffer_idx].as_mut()
    }

    pub fn send_frame<T>(&mut self, qh: &wayland_client::QueueHandle<T>)
    where
        T: wayland_client::Dispatch<wayland_client::protocol::wl_callback::WlCallback, ()> + 'static,
    {
        // Swap buffers
        self.last_buffer = if self.last_buffer == 0 { 1 } else { 0 };

        if let Some(buffer) = &mut self.buffers[self.last_buffer] {
            // Create frame callback
            self.frame_callback = Some(self.surface.frame(qh, ()));

            // Mark buffer as busy
            buffer.busy = true;

            // Damage and attach
            self.surface
                .damage_buffer(0, 0, i32::MAX, i32::MAX);
            self.surface.attach(Some(&buffer.buffer), 0, 0);
            self.surface.set_buffer_scale(self.monitor_scale);
            self.surface.commit();

            self.dirty = false;
            self.rendered = true;
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }
}
