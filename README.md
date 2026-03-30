# Minimg

A minimal, keyboard-driven image viewer built with Rust and GPU-accelerated rendering.

## Features

- GPU-accelerated rendering with wgpu
- Keyboard-first navigation
- Pan, zoom (keyboard, scroll wheel, trackpad pinch-to-zoom), and rotate
- Browse directories of images
- Fullscreen mode
- Wayland and X11 support

## Usage

```
minimg [IMAGE or DIR]...
```

Pass one or more image files or directories. Directories are scanned for supported image formats.

## Keybindings

### Navigation

| Key | Action |
|-----|--------|
| `Space` / `l` / `n` | Next image |
| `Shift+Space` / `h` / `p` / `N` / `Backspace` | Previous image |
| `Home` | First image |
| `End` | Last image |

### View

| Key | Action |
|-----|--------|
| Arrow keys | Pan |
| `=` / scroll up | Zoom in |
| `-` / scroll down | Zoom out |
| Pinch | Zoom (trackpad) |
| `r` | Rotate right |
| `R` | Rotate left |
| `0` | Reset view |
| `f` | Toggle fullscreen |

### Other

| Key | Action |
|-----|--------|
| `c` | Print current file path to stdout |
| `q` / `Escape` | Quit |

## Building

```
cargo build --release
```

## Installation

To make `minimg` available system-wide, symlink the binary into a directory on your `PATH`:

```
ln -sf "$(pwd)/target/release/minimg" ~/.local/bin/minimg
```

## License

[GNU General Public License v3.0 or later](https://www.gnu.org/licenses/gpl-3.0-standalone.html)
