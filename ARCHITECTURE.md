# Architecture and Design Decisions

This document describes the architecture, design patterns, and technical decisions in remagnify.

## Table of Contents

1. [Overview](#overview)
2. [Core Architecture](#core-architecture)
3. [Module Structure](#module-structure)
4. [Wayland Integration](#wayland-integration)
5. [Rendering Pipeline](#rendering-pipeline)
6. [Coordinate System](#coordinate-system)
7. [Memory Management](#memory-management)
8. [Design Decisions](#design-decisions)
9. [Performance Considerations](#performance-considerations)

## Overview

Remagnify is a Wayland screen magnifier built on the wlroots protocol extensions. It's designed as a direct port of hyprmagnifier from C++ to Rust, leveraging Rust's memory safety guarantees while maintaining the same core functionality.

### Key Technologies

- **Language**: Rust (2021 edition)
- **Wayland Client**: wayland-client crate with Dispatch trait system
- **Graphics**: Cairo for 2D rendering
- **Build System**: Cargo with wayland-scanner integration

## Core Architecture

### Event-Driven Design

Remagnify uses an event-driven architecture centered around Wayland's event loop:

```
┌─────────────────────────────────────────────┐
│         Wayland Event Loop                  │
│  (wayland_client::EventQueue)               │
└──────────────┬──────────────────────────────┘
               │
               ├─→ Compositor Events (globals, registry)
               ├─→ Output Events (monitor configuration)
               ├─→ Pointer Events (motion, enter, leave, scroll)
               ├─→ Keyboard Events (key press, escape)
               ├─→ Surface Events (configure, frame, etc.)
               └─→ Screencopy Events (ready, failed, buffer done)
                   │
                   ├─→ Update monitor state
                   ├─→ Capture screen content
                   ├─→ Render magnified view
                   └─→ Present to compositor
```

### State Management

The application state is centralized in `AppState`:

```rust
pub struct AppState {
    // Wayland globals
    compositor: Option<WlCompositor>,
    shm: Option<WlShm>,
    layer_shell: Option<ZwlrLayerShellV1>,
    screencopy_manager: Option<ZwlrScreencopyManagerV1>,

    // Application state
    monitors: Vec<Monitor>,
    layer_surfaces: Vec<LayerSurface>,
    renderer: Renderer,

    // Pointer tracking
    active_monitor: Option<usize>,
    magnifier_position: Vector2D,
    pointer_position_confirmed: bool,

    // Lifecycle
    running: Arc<AtomicBool>,
    initialization_complete: bool,
}
```

This centralized state makes it easy to:
- Track the current state across all Wayland events
- Coordinate between different protocol objects
- Ensure consistent behavior across monitors

### Dispatch Trait System

Remagnify implements Wayland's `Dispatch` trait for each protocol object:

```rust
impl Dispatch<WlPointer, ()> for AppState {
    fn event(
        state: &mut Self,
        _pointer: &WlPointer,
        event: <WlPointer as Proxy>::Event,
        ...
    ) {
        match event {
            Event::Motion { surface_x, surface_y, .. } => {
                // Handle pointer motion
            }
            Event::Button { button, state, .. } => {
                // Handle button clicks
            }
            Event::Axis { axis, value, .. } => {
                // Handle scroll wheel (zoom)
            }
            ...
        }
    }
}
```

This pattern provides:
- Type-safe event handling
- Clear separation of concerns
- Easy-to-follow event flow

## Module Structure

### main.rs - Entry Point

- CLI argument parsing (clap)
- Logger initialization
- Signal handlers (Ctrl+C)
- Magnifier creation and run

### magnifier.rs - Core Event Loop

The main application logic:

- Wayland connection establishment
- Global object binding
- Event queue management
- Dispatch implementations for all Wayland protocols
- Main event loop with graceful shutdown

**Key Functions**:
- `Magnifier::run()`: Main event loop
- `render_surface()`: Coordinates screencopy and rendering
- Dispatch implementations for: Registry, Output, Seat, Pointer, Keyboard, LayerSurface, ScreencopyFrame

### monitor.rs - Monitor State

Per-monitor state tracking:

```rust
pub struct Monitor {
    pub name: String,
    pub output: WlOutput,
    pub size: Vector2D,
    pub scale: i32,
    pub transform: Transform,
    pub screen_buffer: Option<PoolBuffer>,
    pub layer_surface_idx: Option<usize>,
}
```

Tracks:
- Monitor geometry and position
- DPI scaling information
- Screen capture buffer
- Associated layer surface

### layer_surface.rs - Overlay Windows

Manages the fullscreen overlay on each monitor:

```rust
pub struct LayerSurface {
    pub surface: WlSurface,
    pub layer_surface: Option<ZwlrLayerSurfaceV1>,
    pub monitor_idx: usize,

    // Double buffering
    pub buffers: [Option<PoolBuffer>; 2],
    pub last_buffer: usize,

    // Rendering state
    pub dirty: bool,
    pub rendered: bool,
    pub frame_callback: Option<WlCallback>,
}
```

Implements:
- Double-buffered rendering
- Frame callbacks for vsync
- Monitor-specific configuration

### pool_buffer.rs - Shared Memory

Manages shared memory buffers for zero-copy rendering:

```rust
pub struct PoolBuffer {
    pub buffer: WlBuffer,
    pub data: *mut u8,  // mmap'd memory
    pub size: usize,
    pub stride: u32,
    pub pixel_size: Vector2D,

    // Cairo integration
    cairo_surface: Option<ImageSurface>,
}
```

Key features:
- Memory-mapped file in XDG_RUNTIME_DIR
- On-demand Cairo surface creation
- Automatic cleanup via Drop trait
- Zero-copy buffer sharing with compositor

### renderer.rs - Magnification

Cairo-based rendering pipeline:

```rust
pub struct Renderer {
    pub zoom: f64,  // 0.01 to 1.0
}

impl Renderer {
    pub fn render_surface(...) -> Result<()> {
        // 1. Render background (full screen capture)
        // 2. Render magnified region
        // 3. Draw outline around magnified area
    }
}
```

Rendering stages:
1. **Background**: Scaled-down full screen capture
2. **Magnified Region**: Zoomed section around pointer (Nearest-neighbor filtering)
3. **Outline**: Visual frame around magnified area

### config.rs - Configuration

CLI parsing and configuration management:

```rust
#[derive(Parser)]
pub struct Cli {
    move_type: MoveType,
    size: Option<Vector2D>,
    zoom_speed: f64,
    exit_delay_ms: u64,
    ...
}

pub struct Config {
    // Validated and clamped values
    zoom_speed: f64,      // 0.001 to 1.0
    exit_delay_ms: u64,   // 0 to 5000
    ...
}
```

Includes:
- Input validation
- Range clamping
- Default values
- Comprehensive tests

### utils/ - Utility Modules

**vector.rs**: 2D vector math
- Arithmetic operations (Add, Sub, Mul, Div)
- Floor, round, ceil operations
- Length and normalization (unused but available)
- Comprehensive operator overloading

**input/**: Input handling abstractions (reserved for future use)
- keyboard.rs: XKB keyboard state management
- pointer.rs: Pointer state abstractions

## Wayland Integration

### Protocol Usage

1. **wl_compositor**: Create surfaces
2. **wl_shm**: Shared memory pools
3. **wl_output**: Monitor information
4. **wl_seat**: Input devices
5. **wl_pointer**: Mouse tracking
6. **wl_keyboard**: Keyboard input (Escape to exit)
7. **zwlr_layer_shell_v1**: Fullscreen overlay surfaces
8. **zwlr_screencopy_v1**: Screen capture

### Layer Shell Configuration

```rust
layer_surface.set_layer(Layer::Overlay);  // Above all windows
layer_surface.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
layer_surface.set_anchor(Anchor::Top | Anchor::Right | Anchor::Bottom | Anchor::Left);
layer_surface.set_exclusive_zone(-1);  // Fullscreen
```

This creates a fullscreen overlay that:
- Appears above all windows
- Receives keyboard input (for Escape key)
- Covers the entire monitor
- Doesn't push other windows aside

### Screencopy Workflow

```
1. Request frame from output
   screencopy_manager.capture_output_region(output, ...)

2. Receive buffer requirements
   → Event::BufferDone { format, width, height, stride }

3. Create shared memory buffer
   → PoolBuffer::new(size, format, stride)

4. Attach buffer to frame
   → frame.copy(buffer)

5. Wait for completion
   → Event::Ready { ... } or Event::Failed { ... }

6. If Ready: render using captured data
```

## Rendering Pipeline

### Double Buffering

Each layer surface maintains two buffers:

```rust
pub buffers: [Option<PoolBuffer>; 2],
pub last_buffer: usize,

// Swap buffers
self.last_buffer = 1 - self.last_buffer;
let current_buffer = &mut self.buffers[self.last_buffer];
```

This prevents tearing and allows the compositor to read one buffer while we render to the other.

### Frame Callbacks

```rust
let frame_callback = surface.frame(...);
// Render to buffer
surface.attach(buffer, 0, 0);
surface.commit();
// Wait for frame_callback Done event before rendering next frame
```

Frame callbacks provide vsync synchronization, ensuring we don't render faster than the display can show.

### Magnification Transform

```rust
// Calculate magnified region source
let click_pos = magnifier_pos / output_size * screen_size;
let source_size = magnifier_size / zoom;

// Create transform matrix
let mut matrix = Matrix::identity();
matrix.scale(scale.x / zoom, scale.y / zoom);
matrix.translate(
    click_pos.x - source_size.x / 2.0,
    click_pos.y - source_size.y / 2.0
);

pattern.set_matrix(matrix);
```

This matrix:
1. Scales to account for different screen/output resolutions
2. Applies zoom factor
3. Translates to center the magnified region on the pointer

## Coordinate System

### Three Coordinate Spaces

1. **Global Compositor Coordinates**
   - Absolute positions across all monitors
   - Example: Monitor at (-5120, 0) ranges from x=-5120 to x=0

2. **Surface-Local Coordinates**
   - Relative to each layer surface
   - Always 0,0 to width,height
   - Requires conversion from global

3. **Buffer Coordinates**
   - Screen capture buffer coordinates
   - May be scaled differently from surface

### Coordinate Conversion

```rust
// Global to surface-local
if surface_x < 0.0 {
    local_x = monitor_size.x + surface_x;
} else {
    local_x = surface_x;
}
```

Handles negative coordinates from monitors positioned left of the origin.

## Memory Management

### RAII Pattern

Rust's ownership system ensures proper cleanup:

```rust
impl Drop for PoolBuffer {
    fn drop(&mut self) {
        unsafe {
            munmap(self.data as *mut _, self.size).ok();
        }
        std::fs::remove_file(&self.file_path).ok();
    }
}
```

When a PoolBuffer goes out of scope:
1. Memory is automatically unmapped
2. Temporary file is deleted
3. No manual cleanup required
4. No memory leaks possible

### Unsafe Justification

The project uses `unsafe` in specific, justified cases:

1. **FFI with libc**: `mkstemp`, `mmap`, `munmap`
   - Required for creating shared memory files
   - Properly encapsulated in safe wrapper functions

2. **Cairo surface creation**: `create_for_data_unsafe`
   - Required by Cairo API
   - Lifetime managed by PoolBuffer ownership

3. **File descriptor handling**: `from_raw_fd`, `into_raw_fd`
   - Required for Wayland buffer creation
   - Ownership properly transferred

All `unsafe` blocks are:
- Minimal in scope
- Documented with safety comments
- Encapsulated in safe public APIs

### Send Safety

```rust
unsafe impl Send for PoolBuffer {}
```

Justified because:
- Buffer data is only accessed through Wayland callbacks
- Wayland event loop is single-threaded
- No concurrent access possible

## Design Decisions

### 1. Motion-Only Initial Rendering

**Decision**: Don't render until first Motion event.

**Rationale**:
- Enter events during initialization may have inaccurate coordinates on offset monitors
- Motion events are guaranteed to have accurate pointer positions
- Small UX trade-off (requiring pointer movement) for 100% positioning accuracy

**Alternative Considered**: Use Enter events with coordinate validation
- Rejected because coordinates were within bounds but still inaccurate
- Would result in frame appearing in wrong location initially

See `KNOWN_ISSUES.md` for detailed analysis.

### 2. Single-Snapshot Capture

**Decision**: Use single-frame screencopy, not continuous capture.

**Rationale**:
- Hyprland doesn't provide hooks for efficient continuous capture
- Avoids compositor feedback loops (capture→render→capture→render)
- Better performance and stability
- Sufficient for most magnifier use cases

**Alternative Considered**: Continuous capture with compositor-specific hooks
- Rejected due to Hyprland limitations
- Would reduce portability to other wlroots compositors

### 3. Centralized State

**Decision**: Single `AppState` struct for all application state.

**Rationale**:
- Simplifies state coordination across events
- Makes data dependencies explicit
- Easier to reason about state changes
- Matches Dispatch trait requirements

**Alternative Considered**: Distributed state across multiple structures
- Rejected as it would require complex synchronization
- Would make event handlers more complicated

### 4. Double Buffering

**Decision**: Two buffers per layer surface.

**Rationale**:
- Prevents tearing
- Allows compositor to read while we render
- Standard practice in graphics programming

**Performance**: Minimal memory overhead for significantly better visual quality.

### 5. Nearest-Neighbor Magnification

**Decision**: Use `Filter::Nearest` for magnified region.

**Rationale**:
- Preserves pixel-perfect clarity for UI elements
- No blur or interpolation artifacts
- Better for reading small text

**Alternative**: Bilinear filtering
- Rejected for magnified region (but used for background)
- Would make text harder to read

## Performance Considerations

### Complexity Metrics

- **Cyclomatic Complexity**: Median 5.5, Max 6
- **LoC**: ~2000 lines total
- **Test Coverage**: 11 unit tests covering core functionality

### Optimization Strategies

1. **Lazy Cairo Surface Creation**
   ```rust
   pub fn get_cairo_surface(&mut self) -> Result<&ImageSurface> {
       if self.cairo_surface.is_none() {
           self.cairo_surface = Some(...);
       }
       Ok(self.cairo_surface.as_ref().unwrap())
   }
   ```

2. **Minimal Redraws**
   - Only render when pointer moves or zoom changes
   - Use frame callbacks for vsync

3. **Zero-Copy Buffers**
   - Memory-mapped shared memory
   - Direct compositor access to buffers

4. **Efficient Event Handling**
   - Match-based dispatch
   - No unnecessary allocations in event handlers

### Memory Usage

- Base application: ~2-5 MB
- Per-monitor overhead: ~width × height × 4 bytes × 3 buffers
  - Example for 1920×1080: ~25 MB per monitor
- Scales linearly with monitor resolution and count

## Testing Strategy

### Unit Tests

Focus on pure logic without Wayland dependencies:

- **Config parsing**: CLI argument validation
- **Zoom clamping**: Renderer zoom limits
- **Vector math**: Coordinate calculations
- **Buffer creation**: Shared memory setup

### Integration Testing

Currently manual:
- Multi-monitor setups
- Different compositor configurations
- Edge cases (monitor hot-plug, etc.)

Future: Consider automated Wayland protocol testing with test compositors.

## Future Improvements

### Potential Enhancements

1. **Better Protocol Abstractions**
   - Move protocol-specific code to `protocols/` module
   - Create higher-level abstractions over raw Wayland types

2. **Configuration File Support**
   - TOML/YAML configuration
   - Per-monitor settings

3. **Additional Rendering Modes**
   - Color inversion for accessibility
   - Contrast enhancement
   - Custom filters

4. **Performance Profiling**
   - Add perf markers
   - Optimize hot paths based on real usage

### Architectural Debt

None significant. The codebase is clean and maintainable:
- Clear module boundaries
- Minimal coupling
- Comprehensive documentation
- Zero warnings/lints

## References

- [wayland-client documentation](https://docs.rs/wayland-client/)
- [cairo documentation](https://docs.rs/cairo-rs/)
- [wlroots protocols](https://gitlab.freedesktop.org/wlroots/wlroots/-/tree/master/protocol)
- [Hyprland wiki](https://wiki.hyprland.org/)

## Contributing

When contributing, please maintain:

1. **Zero warnings**: `cargo check` must be clean
2. **Zero clippy lints**: `cargo clippy` must pass
3. **Low complexity**: Keep cyclomatic complexity below 10
4. **Test coverage**: Add tests for new functionality
5. **Documentation**: Update this file for architectural changes
