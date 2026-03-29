# minimg - Status Notes

## Current State (2026-03-28)

Working on Fedora 39, AMD Radeon 780M, GNOME Wayland.

### Dependencies

- `show-image` is used via a local path dependency (`../show-image-rs`).
  Two branches exist:
  - `update-winit` — upgrades winit 0.28→0.30.9, fixes Wayland issues,
    uses storage buffers for rendering (manual bilinear in shader).
  - `use-gpu-textures` — builds on `update-winit`, replaces storage
    buffers with GPU textures + samplers + mipmapping.
- `image` crate updated to 0.25 to match show-image 0.14.1.

### What Works

- **Image display** - opens a window, renders images correctly on Wayland with
  HiDPI (buffer_scale=2). Uses GPU textures with trilinear filtering.
- **Keyboard navigation** - arrows pan when zoomed, h/l/n/N for prev/next
  image, Space/Shift+Space, Backspace, Home/End.
- **Keyboard zoom** - `-` / `=` (with or without Ctrl).
- **Two-finger scroll zoom** - works via `MouseWheel` `PixelDelta` events.
  Falls back to x-axis delta when y is zero (common on Wayland touchpads).
- **Click-and-drag pan** - works well.
- **Arrow key panning** - clamped to image bounds (can't scroll into black).
- **Rotation** - `r` (clockwise), `R` (counter-clockwise).
- **Reset** - `0` resets transforms.
- **Fullscreen** - `f` toggles fullscreen.
- **Directory browsing** - pass a directory to view all images in it, sorted
  by name.
- **Window title** - shows image path relative to cwd.
- **Print path** - `c` prints the current image path to console.
- **Large images** - automatically downscaled if they exceed GPU limits.
- **Image switching** - preserves zoom level, resets pan to top of image.

### What Doesn't Work

- **Pinch-to-zoom** - winit 0.30.x does not bind the Wayland
  `zwp_pointer_gestures_v1` protocol. The `PinchGesture` event exists in
  winit's API but is only functional on macOS/iOS. GNOME itself supports the
  protocol (Firefox, Nautilus, etc. use it via GTK), but winit doesn't request
  it during seat initialization, so the compositor never sends gesture events.
  **Fix available:** [winit PR #4338](https://github.com/rust-windowing/winit/pull/4338)
  adds Wayland gesture support and was merged Sep 2025. It's included in
  winit 0.31.0-beta.1+ but not in any 0.30.x release. Upgrading show-image-rs
  to winit 0.31 would enable pinch-to-zoom.

### Keybindings

| Key              | Action                  |
|------------------|-------------------------|
| `q` / `Escape`   | Quit                    |
| `Space`          | Next image              |
| `Shift+Space`    | Previous image          |
| `Backspace`      | Previous image          |
| `n` / `l`        | Next image              |
| `N` / `h` / `p`  | Previous image          |
| `Home` / `End`   | First / last image      |
| Arrow keys       | Pan (when zoomed in)    |
| `-` / `=`        | Zoom out / in           |
| `0`              | Reset zoom/pan          |
| `r` / `R`        | Rotate right / left     |
| `f`              | Toggle fullscreen       |
| `c`              | Print image path        |

### Fixes Applied

1. **Wayland `wl_surface` event 2 crash** - old wayland-client 0.29.5 couldn't
   handle `preferred_buffer_scale` from newer compositors. Fixed by upgrading
   show-image from 0.13.1 to 0.14.1.

2. **HiDPI buffer_scale crash** - sctk-adwaita 0.5.4 generated title bar
   buffers with dimensions not divisible by buffer_scale (e.g. 820x45 with
   scale=2). Fixed by upgrading to winit 0.30 + sctk-adwaita 0.10.1 on the
   `update-winit` branch of show-image-rs.

3. **Surface timeout crash** - `wgpu::Surface::get_current_texture()` was
   called with `.expect()`, panicking on `Timeout`/`Outdated` errors during
   rapid redraws. Fixed with graceful error handling and surface reconfiguration
   in `show-image-rs/src/backend/context.rs`.

4. **Keyboard API migration** - winit 0.30 removed `VirtualKeyCode`. Migrated
   to `Key`/`NamedKey` API in `src/bin/minimg.rs`.

5. **Presentation mode** - changed from `AutoVsync` to `Mailbox` for better
   interactive performance (`show-image-rs/src/backend/context.rs`).

6. **GPU buffer size crash** - large images exceeded `max_storage_buffer_binding_size`
   (128 MiB). Fixed by auto-downscaling in `src/fs_utils.rs`.

### show-image-rs Changes

All changes are local at `~/dev/show-image-rs`:

**`update-winit` branch:**
- Upgraded winit to 0.30.9 (+ full dependency chain update).
- Exposed keyboard modifiers through event conversion pipeline.
- Graceful wgpu surface error handling instead of panicking.
- Converted winit `PinchGesture` to `TouchpadMagnifyEvent` (ready for when
  winit adds Wayland gesture support).
- Added x-axis fallback for `PixelDelta` scroll zoom.
- Changed present mode to `Mailbox`.

**`use-gpu-textures` branch (builds on `update-winit`):**
- Replaced storage buffer rendering with GPU textures + samplers.
- All pixel formats converted to RGBA8 on CPU before upload.
- Full mipmap chain generated on CPU and uploaded per-level.
- Trilinear filtering (bilinear + mipmap) via wgpu::Sampler.
- Fragment shader reduced from 130 lines to a single `texture()` call.
- Removed `GpuImageUniforms` struct and format-specific shader code.
