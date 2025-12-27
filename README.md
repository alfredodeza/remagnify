# Remagnify

A Rust port of [hyprmagnifier](https://github.com/hyprwm/hyprmagnifier), a wlroots-compatible Wayland screen magnifier for compositors like Hyprland and Sway.

## Features

- **Real-time screen magnification** with mouse tracking
- **Adjustable zoom level** via scroll wheel (0.01x to 1.0x)
- **Two movement modes:**
  - `cursor`: Magnifier follows mouse cursor (default)
  - `corner`: Magnifier moves relative to cursor movement
- **Continuous capture mode** for live screen updates
- **Fractional scaling support** for HiDPI displays
- **Multi-monitor support**
- **Customizable magnifier window size**
- **Keyboard shortcut**: Press `Escape` to exit

## Requirements

- Rust 1.70+ (2021 edition)
- A wlroots-based Wayland compositor (Hyprland, Sway, etc.)
- The following system libraries:
  - `wayland-client`
  - `cairo`
  - `pango`
  - `xkbcommon`

### Required Wayland Protocols

Your compositor must support:
- `wlr-layer-shell-unstable-v1` - For overlay windows
- `wlr-screencopy-unstable-v1` - For screen capture (critical)
- `cursor-shape-v1` - For cursor management
- `fractional-scale-v1` - For fractional scaling (optional)
- `viewporter` - For viewport/scaling support (optional)

## Building

```bash
cargo build --release
```

The binary will be located at `target/release/remagnify`.

## Installation

```bash
cargo install --path .
```

Or copy the binary to your PATH:

```bash
sudo cp target/release/remagnify /usr/local/bin/
```

## Usage

```bash
remagnify [OPTIONS]
```

### Options

- `-m, --move-type <TYPE>` - Movement mode: `cursor` or `corner` (default: `cursor`)
- `-s, --size <WIDTHxHEIGHT>` - Magnifier window size (default: `300x150`)
- `-r, --render-inactive` - Render inactive displays as frozen snapshots
- `-c, --continuous <BOOL>` - Enable continuous capture for live updates (default: `true`)
- `-t, --no-fractional` - Disable fractional scaling
- `-q, --quiet` - Quiet mode (errors only)
- `-v, --verbose` - Verbose logging
- `-h, --help` - Print help information
- `-V, --version` - Print version information

### Examples

```bash
# Start with default settings
remagnify

# Custom magnifier size
remagnify --size 500x250

# Corner movement mode
remagnify --move-type corner

# Disable continuous capture (single screenshot)
remagnify --continuous false

# Verbose logging
remagnify --verbose
```

## How It Works

Remagnify creates fullscreen overlay windows on each monitor using the `wlr-layer-shell` protocol. It continuously captures the screen content via `wlr-screencopy` and renders a magnified view using Cairo graphics library. The magnifier follows your mouse cursor and zooms in/out with the scroll wheel.

## Project Structure

```
remagnify/
├── src/
│   ├── main.rs           # Entry point and CLI parsing
│   ├── magnifier.rs      # Main event loop and Wayland connection
│   ├── monitor.rs        # Monitor management
│   ├── layer_surface.rs  # Wayland layer surface handling
│   ├── pool_buffer.rs    # Shared memory buffer management
│   ├── renderer.rs       # Cairo rendering pipeline
│   ├── config.rs         # Configuration and CLI options
│   ├── input/            # Keyboard and pointer input handlers
│   ├── protocols/        # Wayland protocol bindings
│   └── utils/            # Utility modules (Vector2D, etc.)
├── protocols/            # Wayland protocol XML files
├── Cargo.toml            # Rust dependencies
└── build.rs              # Build-time protocol generation
```

## Architecture

Remagnify uses Rust's `wayland-client` library with a `Dispatch`-based event system:

- **Event-driven**: Wayland events are processed in a main event loop
- **Double-buffering**: Two buffers per surface to prevent tearing
- **Memory-mapped buffers**: Zero-copy buffer sharing with the compositor
- **Cairo rendering**: Hardware-accelerated 2D graphics
- **RAII cleanup**: Automatic resource management via Rust's `Drop` trait

## Differences from Hyprmagnifier

- Uses Rust's ownership system instead of C++ smart pointers
- `Dispatch` traits instead of C++ callbacks
- Memory safety guaranteed at compile time
- `Result`-based error handling instead of return codes
- Cargo build system instead of CMake

## Development Status

⚠️ **Work in Progress**: This is a functional port of the core functionality from hyprmagnifier. The basic event loop and Wayland connection are implemented, but full feature parity requires:

- Complete screencopy protocol integration
- Layer shell surface configuration
- Full input event handling
- Signal handling for graceful shutdown
- Comprehensive testing on different compositors

## Contributing

Contributions are welcome! Areas that need work:

1. Complete Wayland protocol event handlers
2. Implement screencopy frame processing
3. Add proper error handling and logging
4. Test on various compositors (Hyprland, Sway, etc.)
5. Performance optimization
6. Documentation improvements

## License

This project maintains the same license as the original hyprmagnifier project.

## Credits

- **Original Project**: [hyprmagnifier](https://github.com/hyprwm/hyprmagnifier) by the Hyprland team
- **Rust Port**: remagnify

## See Also

- [Hyprland](https://hyprland.org/) - Dynamic tiling Wayland compositor
- [wlroots](https://gitlab.freedesktop.org/wlroots/wlroots) - Modular Wayland compositor library
- [wayland-rs](https://github.com/Smithay/wayland-rs) - Rust bindings for Wayland
