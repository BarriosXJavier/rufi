# rufi

A fast, highly customizable application launcher for X11 built in Rust.

## Features

-  **Ultra-fast fuzzy search** with intelligent scoring
-  **Highly customizable** themes with multiple presets
-  **Desktop applications support** (.desktop file parsing)
-  **Proper keyboard handling** with X11 keymap detection  
-  **Smart ranking** (applications preferred over commands)
-  **Intelligent caching** with configurable timeout
-  **Optimized performance** with background loading
-  **Beautiful UI** with Catppuccin theme by default

## Installation

### Prerequisites

- Rust 1.70+ 
- X11 development libraries

**Ubuntu/Debian:**
```bash
sudo apt install build-essential libx11-dev libxcb1-dev
```

**Fedora/RHEL:**
```bash
sudo dnf install gcc libX11-devel libxcb-devel
```

**Arch Linux:**
```bash
sudo pacman -S base-devel libx11 libxcb
```

### Build from source

```bash
git clone https://github.com/BarriosXJavier/rufi.git
cd rufi
cargo build --release
sudo cp target/release/rufi /usr/local/bin/
```

## Configuration

Create `~/.config/rufi/rufirc.toml` to customize rufi. The configuration file uses TOML format:

```toml
# Window settings
width = 800
height = 500
font = "JetBrains Mono"
font_size = 14

# Performance
max_results = 50
cache_timeout = 300

# Display
show_descriptions = true
show_icons = true

[theme]
bg_color = 0x1e1e2e
fg_color = 0xcdd6f4
selected_bg = 0x89b4fa
# ... more theme options

Note: These theme options correspond to the `ConfigTheme` struct in the source code.
```

### Available Themes

rufi comes with several built-in themes, with light and dark variations:
- `catppuccin-mocha` (default)
- `catppuccin-latte`
- `nord-dark`
- `nord-light`
- `dracula`
- `tokyo-night-dark`
- `tokyo-night-light`
- `gruvbox-dark`
- `gruvbox-light`

You can list all available themes with the `--available-themes` flag.

## Usage

### Basic Usage

```bash
# Launch rufi
rufi

# Or bind to a key combination in your window manager
# Example for i3/sway: bindsym $mod+d exec rufi
```

### Command-line Options

You can set and save the default theme using the `--theme` flag:

```bash
rufi --theme nord-dark
```
This command will update your `~/.config/rufi/rufirc.toml` file with the selected theme and then launch rufi with the new theme applied.

To see a list of all available themes, run:

```bash
rufi --available-themes
```

### Keyboard Controls

- **Type**: Search applications and commands
- **↑/↓**: Navigate results
- **Enter**: Launch selected item
- **Escape**: Close rufi
- **Backspace**: Delete characters

### Search Features

rufi provides intelligent fuzzy search with multiple matching strategies:

1. **Exact matches** - highest priority
2. **Prefix matches** - `fir` matches `firefox`  
3. **Substring matches** - `fox` matches `firefox`
4. **Description matches** - searches in app descriptions
5. **Fuzzy matches** - `ff` matches `firefox`

Applications are prioritized over command-line tools in search results.

## Performance Optimizations

- **Background loading**: Applications load asynchronously
- **Smart caching**: Configurable cache timeout prevents unnecessary rescanning
- **Optimized fuzzy search**: Fast scoring algorithm with early termination
- **Minimal redraws**: Only updates when necessary
- **Efficient X11 usage**: Proper resource management

## Development

### Project Structure

```
src/
├── main.rs           # Main application logic
├── config.rs         # Configuration handling  
├── fuzzy.rs          # Fuzzy search algorithms
├── ui.rs             # X11 UI rendering
├── commands.rs       # Application/command collection
├── error.rs          # Custom error types
└── theme.rs          # Built-in theme definitions
```

### Building for Development

```bash
# Debug build with faster compilation
cargo build

# Run with debug logging
RUST_LOG=debug cargo run

# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```


## Contributing

1. Fork the repository
2. Create a feature branch: `git checkout -b feature-name`
3. Make your changes with tests
4. Run `cargo fmt` and `cargo clippy`
5. Submit a pull request

### Feature Requests

- [ ] Custom font support via Pango/Cairo for anti-aliased text
- [ ] Icon rendering support
- [ ] Plugin system
- [ ] Wayland support
- [ ] SSH/remote command execution
- [ ] File browser mode
- [ ] Window switcher mode

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Acknowledgments

- Inspired by [rofi](https://github.com/davatorium/rofi)
- Theme colors from [Catppuccin](https://github.com/catppuccin/catppuccin)
- Built with [x11rb](https://github.com/psychon/x11rb) for X11 bindings

---

**Made with ❤️ and ☕ for the Linux desktop**
