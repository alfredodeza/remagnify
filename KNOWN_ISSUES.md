# Known Issues and Design Decisions

This document explains the technical challenges, design decisions, and known limitations of remagnify.

## 1. Initial Render Delay

### Behavior

The magnifying frame does not render immediately when the application starts. A small pointer movement (even 1 pixel) is required to trigger the initial render.

### Root Cause

During Wayland surface initialization, Hyprland's `wl_pointer::Enter` events may contain calculated/estimated coordinates rather than the actual pointer position. This works accidentally for monitors at the origin (0,0) but fails for offset monitors:

- **Monitor 0 (at origin 0,0)**: Enter events report accurate coordinates because no transformation is needed
- **Monitor 1 (offset, e.g., -5120,0)**: Enter events may report coordinates near the monitor's center (~2777 for a 5120-wide monitor) instead of the actual pointer position (e.g., 4699)

### Investigation Summary

Multiple fix attempts were made:

1. **Attempt 1**: Render immediately when screencopy completes (like hyprmagnifier in C++)
   - Result: Frame appeared on wrong monitor when starting on offset monitors

2. **Attempt 2**: Use the first Enter event during initialization
   - Result: Sometimes selected wrong monitor (last Enter wins, not first)

3. **Attempt 3**: Use first Enter event with coordinate validation
   - Result: Coordinates were within bounds but still inaccurate (2777 vs actual 4699)

4. **Final Solution**: Wait for first Motion event
   - Motion events provide 100% accurate pointer coordinates
   - Trade-off: Requires tiny pointer movement but guarantees accuracy across all monitors
   - This approach was explicitly chosen after understanding the limitations

### Technical Details

```
Example logs from monitor 1 (5120x1440 @ -5120,0):

Initial attempt (coordinate validation):
→ ✓ Saved FIRST valid Enter: monitor 1 at (2777.7, 609.6)
→ Actual pointer location: (4699, 609) - 1922 pixels off!
→ Frame rendered near center, jumps to correct position on motion

Current approach (Motion-only):
→ Waiting for first Motion event to ensure accurate coordinates
→ Motion event: (4699.0, 609.0) - exact pointer position ✓
→ Frame renders at correct location immediately
```

### Why This Happens

Hyprland uses a global compositor coordinate system. During surface initialization:
- Enter events may use estimated/default coordinates for complex monitor layouts
- The transformation from global→surface-local coordinates isn't finalized
- Motion events occur after initialization and provide actual pointer tracking data

### User Impact

**Minimal**: Users only need to wiggle the mouse slightly when starting remagnify. Once the first motion is detected, the magnifier tracks the pointer perfectly in real-time.

## 2. No Live Preview During Magnification

### Behavior

The magnified content shows a static snapshot, not a live updating view of the screen.

### Root Cause

Hyprland (and wlroots compositors in general) don't provide the necessary hooks for efficient continuous screen capture while the magnifier overlay is active:

1. **Feedback Loop Issue**: Using `wlr-screencopy` while the magnifier is rendering creates a capture→render→capture→render loop
2. **Performance Impact**: Continuous screencopy at 60+ FPS would significantly impact compositor performance
3. **Protocol Limitations**: The screencopy protocol is designed for single-frame captures, not video streaming
4. **Layer Surface Interference**: The magnifier's overlay layer interferes with capturing the "true" screen content beneath it

### Investigation Summary

Initial development attempted to implement live preview:
- Continuous capture while magnifying was poorly executed
- Hyprland doesn't provide the necessary compositor hooks
- The feature was abandoned in favor of the current stable single-snapshot approach

### Alternatives Considered

1. **Compositor Plugin**: Would require Hyprland-specific plugin to capture before overlay rendering
   - Breaks portability to other wlroots compositors
   - Requires maintaining compositor-specific code

2. **Separate Capture Layer**: Using a different rendering layer for capture
   - Still suffers from feedback loop issues
   - Doesn't solve the fundamental protocol limitation

3. **Daemon Mode with IPC**: Running as a background daemon and capturing continuously
   - High resource usage (continuous screen capture)
   - Still doesn't solve the overlay interference problem
   - Would help with initial positioning but not live preview

### Current Behavior

The magnifier captures a single frame when:
- The application starts (after first pointer motion)
- The pointer moves to a new area of the screen
- The user scrolls to change zoom level

This provides a good balance of functionality and performance for most use cases.

### User Impact

**Low to Moderate**: For most use cases (reading small text, examining UI details, etc.), a static snapshot works well. The limitation becomes noticeable when trying to magnify video content or animations.

## 3. Coordinate System Complexity

### Technical Details

Remagnify must handle multiple coordinate systems:

1. **Global Compositor Coordinates**: Hyprland's absolute coordinate space
   - Monitor 0 (1920x1080): 0,0 to 1920,1080
   - Monitor 1 (5120x1440 @ -5120,0): -5120,0 to 0,1440

2. **Surface-Local Coordinates**: Relative to each layer surface
   - Always 0,0 to surface_width,surface_height
   - Must be converted from global coordinates

3. **Screen Buffer Coordinates**: The captured screen content
   - May be scaled differently than the output surface
   - Requires careful transformation matrix calculations

### Conversion Logic

```rust
// From global to surface-local (for negative coordinates)
if surface_x < 0.0 {
    local_x = monitor_size.x + surface_x;
} else {
    local_x = surface_x;
}
```

This complexity is why the initial Enter event coordinate validation wasn't sufficient - the coordinates were "valid" (within bounds) but still inaccurate due to incomplete transformation during initialization.

## 4. Multi-Monitor Edge Cases

### Known Limitations

1. **Monitor Arrangement**: Works best with horizontal monitor layouts
   - Vertical stacking may have minor edge cases at boundaries
   - Diagonal arrangements are untested

2. **Dynamic Monitor Changes**: Hot-plugging monitors while remagnify is running is untested
   - Recommendation: Restart remagnify after monitor configuration changes

3. **Mixed DPI/Scaling**: Multi-monitor setups with different scaling factors
   - Basic support via fractional scaling protocol
   - Complex mixed-DPI scenarios may have rendering artifacts

## 5. Design Philosophy: Toyota Way Principles

The development of remagnify followed Toyota Way principles:

1. **Root Cause Analysis**: Extensive investigation of the initial render issue before accepting the Motion-only solution
2. **Quality First**: Zero warnings, comprehensive tests, low cyclomatic complexity
3. **Pragmatic Trade-offs**: Accepting the motion-required initial render over potential coordinate inaccuracy
4. **Simplicity**: Choosing single-snapshot over complex live preview implementation
5. **Continuous Improvement**: Open to better solutions as Wayland protocols and Hyprland evolve

## Future Improvements

Potential areas for improvement if Wayland/Hyprland protocols evolve:

1. **Better Initial Positioning**: If Hyprland provides accurate Enter coordinates during initialization
2. **Live Preview Support**: If screencopy protocol gains streaming capabilities or compositors provide dedicated magnifier APIs
3. **Hot-plug Monitor Support**: Better handling of dynamic monitor configuration changes
4. **Advanced Coordinate Handling**: Better support for complex multi-monitor arrangements

## Contributing

If you encounter issues not listed here or have ideas for improvements, please:

1. Check if the issue is already documented here
2. Test on the latest version of Hyprland and remagnify
3. Provide detailed logs with `--verbose` flag
4. Open an issue with reproduction steps

## References

- [wlroots screencopy protocol](https://wayland.app/protocols/wlr-screencopy-unstable-v1)
- [wlroots layer-shell protocol](https://wayland.app/protocols/wlr-layer-shell-unstable-v1)
- [Hyprland documentation](https://wiki.hyprland.org/)
- [Original hyprmagnifier](https://github.com/hyprwm/hyprmagnifier)
