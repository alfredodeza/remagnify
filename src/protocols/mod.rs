// Re-export Wayland protocol bindings from wayland-protocols-wlr crate
// Reserved for future use when abstracting protocol interactions

#[allow(unused_imports)]
pub use wayland_protocols_wlr::layer_shell::v1::client as wlr_layer_shell;
#[allow(unused_imports)]
pub use wayland_protocols_wlr::screencopy::v1::client as wlr_screencopy;
