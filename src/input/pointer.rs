use crate::utils::Vector2D;
use wayland_client::protocol::wl_pointer::WlPointer;

pub struct Pointer {
    pub pointer: WlPointer,
    pub position: Vector2D,
    pub entered: bool,
}

impl Pointer {
    pub fn new(pointer: WlPointer) -> Self {
        Self {
            pointer,
            position: Vector2D::default(),
            entered: false,
        }
    }

    pub fn handle_enter(&mut self, surface_x: f64, surface_y: f64) {
        self.position = Vector2D::new(surface_x, surface_y);
        self.entered = true;
    }

    pub fn handle_leave(&mut self) {
        self.entered = false;
    }

    pub fn handle_motion(&mut self, surface_x: f64, surface_y: f64) {
        self.position = Vector2D::new(surface_x, surface_y);
    }
}
