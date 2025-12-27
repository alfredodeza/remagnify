fn main() {
    // Note: We're using the wayland-protocols-wlr crate which already provides
    // the generated bindings for wlr-layer-shell and wlr-screencopy protocols.
    // If custom protocol generation is needed, wayland-scanner can be used.

    println!("cargo:rerun-if-changed=protocols/");
}
