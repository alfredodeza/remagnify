use crate::config::Config;
use crate::layer_surface::LayerSurface;
use crate::monitor::Monitor;
use crate::renderer::Renderer;
use crate::utils::Vector2D;
use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wayland_client::protocol::{
    wl_compositor::WlCompositor, wl_keyboard::WlKeyboard, wl_output::WlOutput,
    wl_pointer::WlPointer, wl_registry, wl_seat::WlSeat, wl_shm::WlShm,
};
use wayland_client::{Connection, Dispatch, QueueHandle};

pub struct Magnifier {
    config: Config,
    running: Arc<AtomicBool>,
}

impl Magnifier {
    #[allow(dead_code)]
    pub fn get_running(&self) -> Arc<AtomicBool> {
        self.running.clone()
    }
}

// Application state for Dispatch implementations
pub struct AppState {
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    seat: Option<WlSeat>,
    layer_shell: Option<wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1>,
    screencopy_manager: Option<wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1>,
    monitors: Vec<Monitor>,
    layer_surfaces: Vec<LayerSurface>,
    next_output_id: u32,

    // Track screencopy frames
    pending_frames: Vec<(ZwlrScreencopyFrameV1, usize)>, // (frame, monitor_idx)

    // Magnifier state
    magnifier_position: Vector2D,
    magnifier_size: Vector2D,
    zoom: f64,
    zoom_speed: f64,
    exit_delay_ms: u64,
    active_monitor: Option<usize>, // Which monitor the cursor is currently on

    // Renderer
    renderer: Renderer,

    // Control
    running: Arc<AtomicBool>,

    // Track initial render
    initial_render_done: bool,

    // Track if we've received a reliable pointer position from Motion event
    // The initial Enter event during startup may be unreliable/arbitrary
    pointer_position_confirmed: bool,

    // Track if initialization is complete
    // During init, we get spurious Enter/Leave events as surfaces are mapped
    initialization_complete: bool,

    // Track the first VALID Enter event during initialization
    // Only Enter events with coordinates within monitor bounds are saved
    first_enter_during_init: Option<(usize, f64, f64)>, // (monitor_idx, x, y)
}

impl Magnifier {
    pub fn new(config: Config) -> Result<Self> {
        // Set up signal handlers
        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        ctrlc::set_handler(move || {
            log::info!("Received Ctrl+C, exiting...");
            r.store(false, Ordering::SeqCst);
        })
        .context("Error setting Ctrl-C handler")?;

        Ok(Self {
            config,
            running,
        })
    }

    pub fn run(&mut self) -> Result<()> {
        log::info!("Connecting to Wayland...");

        // Connect to Wayland
        let conn = Connection::connect_to_env()
            .context("Failed to connect to Wayland compositor")?;

        let display = conn.display();
        let mut event_queue = conn.new_event_queue();
        let qh = event_queue.handle();

        // Create initial state
        let mut state = AppState {
            compositor: None,
            shm: None,
            seat: None,
            layer_shell: None,
            screencopy_manager: None,
            monitors: Vec::new(),
            layer_surfaces: Vec::new(),
            next_output_id: 0,
            pending_frames: Vec::new(),
            magnifier_position: Vector2D::new(500.0, 500.0), // Default position
            magnifier_size: self.config.size,
            zoom: 0.5, // 2x zoom (zoom = 0.5 means we show half the area, effectively 2x magnification)
            zoom_speed: self.config.zoom_speed,
            exit_delay_ms: self.config.exit_delay_ms,
            active_monitor: None, // Will be set when pointer enters a surface
            renderer: Renderer::new(),
            running: self.running.clone(),
            initial_render_done: false,
            pointer_position_confirmed: false,
            initialization_complete: false,
            first_enter_during_init: None,
        };

        // Get registry
        let _registry = display.get_registry(&qh, ());

        // Initial roundtrip to get globals
        event_queue
            .roundtrip(&mut state)
            .context("Failed initial roundtrip")?;

        log::info!("Connected to Wayland compositor");
        log::info!("Found {} monitors", state.monitors.len());

        // Additional roundtrip to let monitors receive their configuration and seat capabilities
        event_queue
            .roundtrip(&mut state)
            .context("Failed to configure monitors and seat")?;

        // Log monitor info
        for (idx, monitor) in state.monitors.iter().enumerate() {
            log::info!(
                "Monitor {}: {}x{} scale={} ({})",
                idx,
                monitor.size.x as i32,
                monitor.size.y as i32,
                monitor.scale,
                monitor.name
            );
        }

        // Another roundtrip to ensure pointer/keyboard objects are created
        event_queue
            .roundtrip(&mut state)
            .context("Failed to setup input devices")?;

        // Check required protocols
        if state.compositor.is_none() {
            anyhow::bail!("Compositor not available");
        }
        if state.shm.is_none() {
            anyhow::bail!("SHM not available");
        }
        if state.layer_shell.is_none() {
            anyhow::bail!("Layer shell not available - your compositor doesn't support wlr-layer-shell");
        }
        if state.screencopy_manager.is_none() {
            anyhow::bail!("Screencopy not available - your compositor doesn't support wlr-screencopy");
        }

        log::info!("All required protocols available - setting up surfaces...");

        // Create layer surfaces for each monitor
        let compositor = state.compositor.as_ref().unwrap();
        let layer_shell = state.layer_shell.as_ref().unwrap();

        for (idx, monitor) in state.monitors.iter_mut().enumerate() {
            log::info!("Creating layer surface for monitor {}", idx);

            // Create Wayland surface
            let surface = compositor.create_surface(&qh, ());

            // Create layer surface
            use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
            let layer_surface = layer_shell.get_layer_surface(
                &surface,
                Some(&monitor.output),
                Layer::Overlay,
                "remagnify".to_string(),
                &qh,
                (),
            );

            // Configure the layer surface
            use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{Anchor, KeyboardInteractivity};
            layer_surface.set_anchor(Anchor::Top | Anchor::Right | Anchor::Bottom | Anchor::Left);
            layer_surface.set_exclusive_zone(-1);
            // OnDemand allows keyboard focus when we need it (for Escape key)
            layer_surface.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);

            surface.commit();

            log::info!("Layer surface {} created and configured", idx);

            // Create LayerSurface wrapper
            let mut ls = LayerSurface::new(
                idx,
                surface,
                monitor.size,
                monitor.scale,
            );
            ls.layer_surface = Some(layer_surface);
            state.layer_surfaces.push(ls);
            monitor.layer_surface_idx = Some(idx);
        }

        // Sync to get configure events and acknowledge them
        event_queue.roundtrip(&mut state)?;

        log::info!("All surfaces created and configured");

        // Verify all surfaces are configured
        let all_configured = state.layer_surfaces.iter().all(|ls| ls.configured);
        if !all_configured {
            anyhow::bail!("Not all layer surfaces were configured");
        }

        // Create and attach initial transparent buffers so surfaces receive input
        log::info!("Creating initial buffers for layer surfaces...");
        let shm = state.shm.as_ref().unwrap();

        for layer_surface in &mut state.layer_surfaces {
            let pixel_size = layer_surface.monitor_size;
            let stride = (pixel_size.x as u32) * 4; // ARGB32 = 4 bytes per pixel
            let format = wayland_client::protocol::wl_shm::Format::Argb8888 as u32;

            // Create both buffers
            for i in 0..2 {
                use crate::pool_buffer::PoolBuffer;
                let mut buffer = PoolBuffer::new(pixel_size, format, stride, shm, &qh)?;

                // Fill with transparent pixels
                let ctx = buffer.create_cairo_context()?;
                ctx.save()?;
                ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
                ctx.set_operator(cairo::Operator::Source);
                ctx.paint()?;
                ctx.restore()?;

                layer_surface.buffers[i] = Some(buffer);
            }

            // Attach and commit the first buffer to map the surface
            layer_surface.send_frame(&qh);
            log::info!("Layer surface {} mapped with initial buffer", layer_surface.monitor_idx);
        }

        conn.flush()?;
        event_queue.roundtrip(&mut state)?;

        // Mark initialization as complete
        state.initialization_complete = true;

        // Set initial state from Enter events if available
        // But DON'T confirm position yet - wait for Motion to ensure accuracy
        if let Some((monitor_idx, x, y)) = state.first_enter_during_init {
            state.active_monitor = Some(monitor_idx);
            state.magnifier_position = Vector2D::new(x, y);
            // pointer_position_confirmed stays false - will be set by Motion event
            log::info!("→ Initial state from Enter: monitor {} at ({:.1}, {:.1}) - waiting for Motion to confirm", monitor_idx, x, y);
        } else {
            log::info!("→ No Enter events during init - waiting for pointer Motion");
        }

        log::info!("All layer surfaces mapped and ready for input");

        // Start screencopy for each monitor
        let screencopy_mgr = state.screencopy_manager.as_ref().unwrap();
        for (idx, monitor) in state.monitors.iter().enumerate() {
            log::info!("Starting screencopy for monitor {}", idx);

            // Capture the output (with overlay_cursor = 0 to not include cursor)
            let frame = screencopy_mgr.capture_output(0, &monitor.output, &qh, ());

            // Track this frame
            state.pending_frames.push((frame, idx));

            log::debug!("Screencopy frame requested for monitor {}", idx);
        }

        // Flush and process initial screencopy events
        conn.flush()?;
        event_queue.roundtrip(&mut state)?;

        log::info!("Screencopy initialized for all monitors");

        // Main event loop
        log::info!("Starting event loop...");
        log::info!("Press Ctrl+C to exit");

        loop {
            // Check if we should exit
            if !self.running.load(Ordering::SeqCst) {
                log::info!("Shutting down...");
                break;
            }

            // Dispatch pending events
            match event_queue.dispatch_pending(&mut state) {
                Ok(_) => {},
                Err(e) => {
                    log::error!("Failed to dispatch events: {}", e);
                    break;
                }
            }

            // Flush the connection
            if let Err(e) = conn.flush() {
                log::error!("Failed to flush connection: {}", e);
                break;
            }

            // Try to read events with proper error handling
            if let Some(guard) = event_queue.prepare_read() {
                // Use poll to wait for events with timeout
                use std::os::unix::io::AsRawFd;
                use nix::libc;
                let fd = guard.connection_fd().as_raw_fd();

                // Poll with 100ms timeout
                let mut pollfd = libc::pollfd {
                    fd,
                    events: libc::POLLIN,
                    revents: 0,
                };

                let poll_result = unsafe { libc::poll(&mut pollfd, 1, 100) };

                if poll_result > 0 {
                    // Data is available to read
                    if let Err(e) = guard.read() {
                        log::error!("Failed to read events: {}", e);
                        break;
                    }
                } else if poll_result < 0 {
                    log::error!("Poll error");
                    break;
                } else {
                    // Timeout - no events available, cancel the read
                    drop(guard);
                }
            }
        }

        log::info!("Event loop terminated");
        Ok(())
    }

}

impl AppState {
    /// Render a monitor's screen buffer to its layer surface
    fn render_monitor<T>(
        &mut self,
        monitor_idx: usize,
        qh: &QueueHandle<T>,
    ) -> Result<()>
    where
        T: wayland_client::Dispatch<WlBuffer, ()> + 'static,
        T: wayland_client::Dispatch<WlShmPool, ()> + 'static,
        T: wayland_client::Dispatch<WlCallback, ()> + 'static,
    {
        // Get the monitor's screen buffer
        let monitor = self.monitors.get_mut(monitor_idx)
            .context("Invalid monitor index")?;

        let screen_buffer = monitor.screen_buffer.as_mut()
            .context("No screen buffer available")?;

        // Find the corresponding layer surface
        let layer_surface = self.layer_surfaces.iter_mut()
            .find(|ls| ls.monitor_idx == monitor_idx)
            .context("No layer surface found for monitor")?;

        if !layer_surface.configured {
            log::warn!("Layer surface {} not configured yet, skipping render", monitor_idx);
            return Ok(());
        }

        // Get or create an output buffer
        let shm = self.shm.as_ref().context("No SHM available")?;

        let output_buffer = layer_surface.get_available_buffer();
        if output_buffer.is_none() {
            // Create new buffers if needed
            log::debug!("Creating output buffers for layer surface {}", monitor_idx);

            let pixel_size = layer_surface.monitor_size;
            let stride = (pixel_size.x as u32) * 4; // ARGB32 = 4 bytes per pixel
            let format = wayland_client::protocol::wl_shm::Format::Argb8888 as u32;

            for i in 0..2 {
                if layer_surface.buffers[i].is_none() {
                    use crate::pool_buffer::PoolBuffer;
                    let buffer = PoolBuffer::new(pixel_size, format, stride, shm, qh)?;
                    layer_surface.buffers[i] = Some(buffer);
                }
            }
        }

        let output_buffer = layer_surface.get_available_buffer()
            .context("No available buffer after creation")?;

        // Sync zoom from AppState to renderer
        self.renderer.set_zoom(self.zoom);

        // Only show magnifier on the active monitor AND if we have a confirmed pointer position
        // We wait for the first Motion event to ensure accurate coordinates (Enter events
        // during initialization can have wrong coordinates for offset monitors)
        let is_active = self.pointer_position_confirmed && self.active_monitor == Some(monitor_idx);

        if is_active {
            // Render the magnified view on the active monitor
            self.renderer.render_surface(
                output_buffer,
                screen_buffer,
                self.magnifier_position,
                self.magnifier_size,
                false, // force_inactive
                false, // render_inactive
            )?;
            log::debug!("Rendered magnifier on monitor {} at position {:?}",
                monitor_idx, self.magnifier_position);
        } else {
            // Render inactive (no magnifier) on other monitors
            let ctx = output_buffer.create_cairo_context()?;
            ctx.save()?;
            ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
            ctx.set_operator(cairo::Operator::Source);
            ctx.paint()?;
            ctx.restore()?;
            log::trace!("Cleared inactive monitor {}", monitor_idx);
        }

        // Attach and commit the buffer
        layer_surface.send_frame(qh);

        Ok(())
    }
}

// Dispatch implementation for WlRegistry
impl Dispatch<wl_registry::WlRegistry, ()> for AppState {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            log::debug!("Global: {} v{} (name: {})", interface, version, name);

            match interface.as_str() {
                "wl_compositor" => {
                    let compositor = registry.bind::<WlCompositor, _, _>(name, 4, qh, ());
                    state.compositor = Some(compositor);
                }
                "wl_shm" => {
                    let shm = registry.bind::<WlShm, _, _>(name, 1, qh, ());
                    state.shm = Some(shm);
                }
                "wl_seat" => {
                    let seat = registry.bind::<WlSeat, _, _>(name, 5, qh, ());
                    state.seat = Some(seat);
                }
                "wl_output" => {
                    let output = registry.bind::<WlOutput, _, _>(name, 3, qh, ());
                    let monitor = Monitor::new(output, state.next_output_id);
                    state.next_output_id += 1;
                    state.monitors.push(monitor);
                }
                "zwlr_layer_shell_v1" => {
                    use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
                    let layer_shell = registry.bind::<ZwlrLayerShellV1, _, _>(name, 1, qh, ());
                    state.layer_shell = Some(layer_shell);
                    log::info!("Layer shell available");
                }
                "zwlr_screencopy_manager_v1" => {
                    use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
                    let screencopy_mgr = registry.bind::<ZwlrScreencopyManagerV1, _, _>(name, 3, qh, ());
                    state.screencopy_manager = Some(screencopy_mgr);
                    log::info!("Screencopy manager available");
                }
                _ => {}
            }
        }
    }
}

// Basic Dispatch implementations for required protocols
impl Dispatch<WlCompositor, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlCompositor,
        _: <WlCompositor as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShm, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlShm,
        _: <WlShm as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlSeat, ()> for AppState {
    fn event(
        _state: &mut Self,
        seat: &WlSeat,
        event: <WlSeat as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_seat::Event;

        log::debug!("WlSeat event: {:?}", event);

        match event {
            Event::Capabilities { capabilities } => {
                use wayland_client::protocol::wl_seat::Capability;

                // Convert WEnum to u32 for comparison
                let caps: u32 = capabilities.into();
                let pointer_cap: u32 = Capability::Pointer.into();
                let keyboard_cap: u32 = Capability::Keyboard.into();

                log::info!("Seat capabilities: raw={} pointer={} keyboard={}",
                    caps, caps & pointer_cap != 0, caps & keyboard_cap != 0);

                if caps & pointer_cap != 0 {
                    log::info!("Getting pointer from seat...");
                    seat.get_pointer(qh, ());
                    log::info!("Pointer object requested");
                }

                if caps & keyboard_cap != 0 {
                    log::info!("Getting keyboard from seat...");
                    seat.get_keyboard(qh, ());
                    log::info!("Keyboard object requested");
                }
            }
            Event::Name { name } => {
                log::info!("Seat name: {}", name);
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, ()> for AppState {
    fn event(
        state: &mut Self,
        output: &WlOutput,
        event: <WlOutput as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_output::Event;

        // Find the monitor
        let monitor = state
            .monitors
            .iter_mut()
            .find(|m| &m.output == output);

        if let Some(monitor) = monitor {
            match event {
                Event::Geometry {
                    x,
                    y,
                    physical_width,
                    physical_height,
                    ..
                } => {
                    monitor.set_geometry(x, y, physical_width, physical_height);
                }
                Event::Mode {
                    width,
                    height,
                    refresh,
                    ..
                } => {
                    monitor.set_mode(width, height, refresh);
                }
                Event::Scale { factor } => {
                    monitor.set_scale(factor);
                }
                Event::Name { name } => {
                    monitor.set_name(name);
                }
                Event::Done => {
                    monitor.set_done();
                }
                _ => {}
            }
        }
    }
}

impl Dispatch<WlKeyboard, ()> for AppState {
    fn event(
        state: &mut Self,
        _keyboard: &WlKeyboard,
        event: <WlKeyboard as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_keyboard::Event;

        match event {
            Event::Key { key, state: key_state, .. } => {
                use wayland_client::protocol::wl_keyboard::KeyState;
                use wayland_client::WEnum;

                // Only handle key presses, not releases
                if let WEnum::Value(KeyState::Pressed) = key_state {
                    // XKB keycode is key + 8
                    let keycode = key + 8;

                    // Escape key is keycode 9 (XKB_KEY_Escape = 0xff1b, but as keycode it's 9)
                    if keycode == 9 {
                        log::info!("Escape key pressed, exiting...");
                        state.running.store(false, Ordering::SeqCst);
                    }
                }
            }
            Event::Modifiers { .. } => {
                // Handle modifier keys if needed
            }
            Event::Keymap { .. } => {
                // Keymap setup
            }
            _ => {}
        }
    }
}

impl Dispatch<WlPointer, ()> for AppState {
    fn event(
        state: &mut Self,
        _pointer: &WlPointer,
        event: <WlPointer as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _qh: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_pointer::Event;

        log::trace!("WlPointer event: {:?}", event);

        match event {
            Event::Enter { surface, surface_x, surface_y, .. } => {
                // Find which monitor this surface belongs to
                let monitor_idx = state.layer_surfaces.iter()
                    .find(|ls| ls.surface == surface)
                    .map(|ls| ls.monitor_idx);

                if let Some(idx) = monitor_idx {
                    // Get monitor size for coordinate conversion
                    let monitor_size = state.monitors.get(idx).map(|m| m.size)
                        .unwrap_or_else(|| Vector2D::new(1920.0, 1080.0));

                    // Hyprland gives global compositor coordinates instead of surface-local
                    // For monitors left of origin: surface_local = monitor_width + global_x
                    // Example: monitor at [-5120, 0], cursor at -2576 → surface_local = 5120 + (-2576) = 2544
                    let local_x = if surface_x < 0.0 {
                        monitor_size.x + surface_x
                    } else {
                        surface_x
                    };
                    let local_y = if surface_y < 0.0 {
                        monitor_size.y + surface_y
                    } else {
                        surface_y
                    };

                    log::info!("Pointer entered monitor {} at ({}, {}) (raw: {}, {}) - monitor size: {}x{}",
                        idx, local_x, local_y, surface_x, surface_y, monitor_size.x, monitor_size.y);

                    // During initialization, save the first Enter event with VALID coordinates
                    // Invalid coordinates indicate spurious events from surface creation
                    if !state.initialization_complete {
                        // Validate coordinates are within monitor bounds
                        let coords_valid = local_x >= 0.0 && local_x <= monitor_size.x
                            && local_y >= 0.0 && local_y <= monitor_size.y;

                        if coords_valid && state.first_enter_during_init.is_none() {
                            state.first_enter_during_init = Some((idx, local_x, local_y));
                            log::info!("✓ Saved FIRST valid Enter: monitor {} at ({:.1}, {:.1})", idx, local_x, local_y);
                        } else if !coords_valid {
                            log::info!("✗ Ignoring invalid Enter: monitor {} at ({:.1}, {:.1}) outside {}x{}",
                                idx, local_x, local_y, monitor_size.x as i32, monitor_size.y as i32);
                        } else {
                            log::info!("⊘ Ignoring subsequent Enter: monitor {}", idx);
                        }
                        return;
                    }

                    state.active_monitor = Some(idx);
                    state.magnifier_position = Vector2D::new(local_x, local_y);

                    // Note: We don't confirm position from Enter events (even after init)
                    // because they can still be inaccurate. We wait for Motion to confirm.

                    // Render at new pointer position
                    // (magnifier will only show if pointer_position_confirmed is true from Motion)
                    if state.monitors.get(idx).and_then(|m| m.screen_buffer.as_ref()).is_some() {
                        if let Err(e) = Self::render_monitor(state, idx, _qh) {
                            log::error!("Failed to render on entry: {}", e);
                        }
                    }
                } else {
                    log::warn!("Pointer entered unknown surface");
                }
            }
            Event::Leave { .. } => {
                log::info!("Pointer left surface");

                // Clear the magnifier from the monitor we're leaving
                // Set active_monitor to None FIRST so render knows to clear it
                let old_monitor = state.active_monitor;
                state.active_monitor = None;

                if let Some(monitor_idx) = old_monitor {
                    if let Err(e) = Self::render_monitor(state, monitor_idx, _qh) {
                        log::error!("Failed to clear on leave: {}", e);
                    }
                }
            }
            Event::Motion { surface_x, surface_y, .. } => {
                // Motion event provides reliable pointer position
                // Mark position as confirmed on first motion
                if !state.pointer_position_confirmed {
                    state.pointer_position_confirmed = true;
                    log::info!("✓ Pointer position confirmed via Motion - magnifier will now be visible");
                }

                // Convert coordinates (handle Hyprland's global coordinates quirk)
                if let Some(monitor_idx) = state.active_monitor {
                    let monitor_size = state.monitors.get(monitor_idx).map(|m| m.size)
                        .unwrap_or_else(|| Vector2D::new(1920.0, 1080.0));

                    let local_x = if surface_x < 0.0 {
                        monitor_size.x + surface_x
                    } else {
                        surface_x
                    };
                    let local_y = if surface_y < 0.0 {
                        monitor_size.y + surface_y
                    } else {
                        surface_y
                    };

                    state.magnifier_position = Vector2D::new(local_x, local_y);
                    log::trace!("Pointer motion: ({:.0}, {:.0})", local_x, local_y);
                } else {
                    // Fallback if active_monitor not set
                    state.magnifier_position = Vector2D::new(surface_x.abs(), surface_y.abs());
                    log::trace!("Pointer motion: ({:.0}, {:.0})", surface_x, surface_y);
                }

                // Render the magnifier at the new position
                if let Some(monitor_idx) = state.active_monitor {
                    // Only render if screencopy is ready
                    if state.monitors.get(monitor_idx).and_then(|m| m.screen_buffer.as_ref()).is_some() {
                        if let Err(e) = Self::render_monitor(state, monitor_idx, _qh) {
                            log::error!("Failed to render on motion: {}", e);
                        }
                    }
                }
            }
            Event::Button { .. } => {
                // Handle button clicks if needed
            }
            Event::Axis { axis, value, .. } => {
                // Handle scroll wheel for zoom
                use wayland_client::protocol::wl_pointer::Axis;
                use wayland_client::WEnum;
                if let WEnum::Value(Axis::VerticalScroll) = axis {
                    let delta = -value / 120.0; // Normalize scroll delta
                    state.zoom = (state.zoom + delta * state.zoom_speed).clamp(0.01, 1.0);
                    state.renderer.set_zoom(state.zoom);
                    log::info!("Zoom adjusted to {:.2}x (zoom factor: {:.2})", 1.0 / state.zoom, state.zoom);

                    // Exit when zoomed all the way out (no magnification)
                    if state.zoom >= 1.0 {
                        log::info!("Zoomed to 1.0 (no magnification), clearing overlay and exiting...");

                        // Clear all overlays first
                        for layer_surface in &mut state.layer_surfaces {
                            if let Some(buffer) = layer_surface.get_available_buffer() {
                                if let Ok(ctx) = buffer.create_cairo_context() {
                                    ctx.save().ok();
                                    ctx.set_source_rgba(0.0, 0.0, 0.0, 0.0);
                                    ctx.set_operator(cairo::Operator::Source);
                                    ctx.paint().ok();
                                    ctx.restore().ok();
                                }
                                layer_surface.send_frame(_qh);
                            }
                        }

                        // Wait for exit delay to prevent scroll events from affecting underlying window
                        if state.exit_delay_ms > 0 {
                            log::debug!("Waiting {}ms before exit...", state.exit_delay_ms);
                            std::thread::sleep(std::time::Duration::from_millis(state.exit_delay_ms));
                        }

                        state.running.store(false, Ordering::SeqCst);
                        return;
                    }

                    // Re-render with new zoom level
                    if let Some(monitor_idx) = state.active_monitor {
                        if let Err(e) = Self::render_monitor(state, monitor_idx, _qh) {
                            log::error!("Failed to render on zoom: {}", e);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_callback::WlCallback;

impl Dispatch<WlCallback, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlCallback,
        _: <WlCallback as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlShmPool, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlShmPool,
        _: <WlShmPool as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlBuffer, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlBuffer,
        event: <WlBuffer as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wayland_client::protocol::wl_buffer::Event;
        if let Event::Release = event {
            // Buffer can be reused
            log::trace!("Buffer released");
        }
    }
}

// Layer shell protocol implementations
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_manager_v1::ZwlrScreencopyManagerV1;
use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::ZwlrScreencopyFrameV1;

impl Dispatch<ZwlrLayerShellV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &ZwlrLayerShellV1,
        _: <ZwlrLayerShellV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for AppState {
    fn event(
        state: &mut Self,
        layer_surface: &ZwlrLayerSurfaceV1,
        event: <ZwlrLayerSurfaceV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::Event;
        match event {
            Event::Configure { serial, .. } => {
                log::debug!("Layer surface configure: serial={}", serial);

                // Find the matching layer surface and acknowledge
                for ls in &mut state.layer_surfaces {
                    if let Some(ref ls_handle) = ls.layer_surface {
                        if ls_handle == layer_surface {
                            ls_handle.ack_configure(serial);
                            ls.configured = true;
                            ls.ack_serial = serial;
                            log::debug!("Acknowledged configure for surface {}", ls.monitor_idx);
                            break;
                        }
                    }
                }
            }
            Event::Closed => {
                log::warn!("Layer surface closed");
            }
            _ => {}
        }
    }
}

impl Dispatch<ZwlrScreencopyManagerV1, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &ZwlrScreencopyManagerV1,
        _: <ZwlrScreencopyManagerV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ZwlrScreencopyFrameV1, ()> for AppState {
    fn event(
        state: &mut Self,
        frame: &ZwlrScreencopyFrameV1,
        event: <ZwlrScreencopyFrameV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        use wayland_protocols_wlr::screencopy::v1::client::zwlr_screencopy_frame_v1::Event;

        // Find which monitor this frame belongs to
        let monitor_idx = state.pending_frames.iter()
            .find(|(f, _)| f == frame)
            .map(|(_, idx)| *idx);

        match event {
            Event::Buffer { format, width, height, stride } => {
                log::debug!("Screencopy buffer: {}x{} format={:?} stride={}", width, height, format, stride);

                if let Some(idx) = monitor_idx {
                    if let Some(shm) = &state.shm {
                        // Create a buffer to receive the screenshot
                        let pixel_size = Vector2D::new(width as f64, height as f64);

                        // Convert format enum to u32
                        let format_u32: u32 = format.into();

                        match crate::pool_buffer::PoolBuffer::new(
                            pixel_size,
                            format_u32,
                            stride,
                            shm,
                            qh,
                        ) {
                            Ok(buffer) => {
                                log::debug!("Created screencopy buffer for monitor {}", idx);

                                // Copy the screen to our buffer
                                frame.copy(&buffer.buffer);

                                // Store buffer info in monitor
                                if let Some(monitor) = state.monitors.get_mut(idx) {
                                    monitor.screen_buffer = Some(buffer);
                                    monitor.screen_buffer_format = format_u32;
                                }
                            }
                            Err(e) => {
                                log::error!("Failed to create screencopy buffer: {}", e);
                            }
                        }
                    }
                }
            }
            Event::Ready { .. } => {
                log::debug!("Screencopy frame ready for monitor {:?}", monitor_idx);

                // Single capture complete - screen data is now available
                if let Some(idx) = monitor_idx {
                    log::info!("Monitor {} screen capture complete", idx);

                    // Clean up the pending frame
                    state.pending_frames.retain(|(f, _)| f != frame);

                    // Render this monitor immediately when its screencopy is ready
                    // This matches hyprmagnifier's behavior where renderSurface is called
                    // immediately in the Ready callback (Monitor.cpp:113)
                    // The render_monitor function already handles inactive monitors correctly
                    // by rendering them transparent if they're not the active monitor
                    log::debug!("Rendering monitor {} after screencopy complete", idx);

                    match Self::render_monitor(state, idx, qh) {
                        Ok(_) => {
                            log::debug!("Monitor {} rendered successfully", idx);
                            if !state.initial_render_done {
                                state.initial_render_done = true;
                                log::info!("Initial render completed");
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to render monitor {}: {}", idx, e);
                        }
                    }

                    // Note: We don't request another frame - this is a single capture
                    // The magnifier will now use this static capture and update only
                    // the magnified region as the cursor moves
                }
            }
            Event::Failed => {
                log::warn!("Screencopy frame failed for monitor {:?}", monitor_idx);
            }
            _ => {}
        }
    }
}

use wayland_client::protocol::wl_surface::WlSurface;

impl Dispatch<WlSurface, ()> for AppState {
    fn event(
        _: &mut Self,
        _: &WlSurface,
        _: <WlSurface as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
