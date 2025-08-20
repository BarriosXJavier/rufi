#!/bin/bash

# rufi installation script
set -e

REPO_URL="https://github.com/BarriosXJavier/rufi.git"
INSTALL_DIR="/usr/local/bin"
CONFIG_SAMPLE="https://raw.githubusercontent.com/BarriosXJavier/rufi/main/.rufirc"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

check_dependencies() {
    print_status "Checking dependencies..."
    
    # Check for Rust
    if ! command -v cargo &> /dev/null; then
        print_error "Rust/Cargo not found. Please install Rust first:"
        echo "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        exit 1
    fi
    
    # Check for git
    if ! command -v git &> /dev/null; then
        print_error "Git not found. Please install git first."
        exit 1
    fi
    
    # Check for X11 development libraries
    if ! pkg-config --exists x11 2>/dev/null; then
        print_warning "X11 development libraries may not be installed."
        print_status "Install them with:"
        
        if command -v apt &> /dev/null; then
            echo "  sudo apt install libx11-dev libxcb1-dev"
        elif command -v dnf &> /dev/null; then
            echo "  sudo dnf install libX11-devel libxcb-devel"
        elif command -v pacman &> /dev/null; then
            echo "  sudo pacman -S libx11 libxcb"
        else
            echo "  Install X11 development libraries for your distribution"
        fi
        
        read -p "Continue anyway? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            exit 1
        fi
    fi
    
    print_success "Dependencies check passed"
}

install_rufi() {
    print_status "Installing rufi..."
    
    # Create temporary directory
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    # Clone repository
    print_status "Cloning repository..."
    git clone "$REPO_URL" rufi
    cd rufi
    
    # Build in release mode
    print_status "Building rufi (this may take a few minutes)..."
    cargo build --release
    
    # Install binary
    print_status "Installing binary to $INSTALL_DIR..."
    sudo cp target/release/rufi "$INSTALL_DIR/rufi"
    sudo chmod +x "$INSTALL_DIR/rufi"
    
    # Cleanup
    cd /
    rm -rf "$TEMP_DIR"
    
    print_success "rufi installed successfully!"
}

setup_config() {
    CONFIG_PATH="$HOME/.rufirc"
    
    if [ -f "$CONFIG_PATH" ]; then
        print_warning "Configuration file already exists at $CONFIG_PATH"
        read -p "Overwrite? (y/N): " -n 1 -r
        echo
        if [[ ! $REPLY =~ ^[Yy]$ ]]; then
            return
        fi
    fi
    
    print_status "Setting up configuration file..."
    
    cat > "$CONFIG_PATH" << 'EOF'
# rufi configuration file
font = "JetBrains Mono"
font_size = 14
width = 800
height = 500
item_height = 32
padding = 16
border_width = 2
corner_radius = 12
max_results = 50
cache_timeout = 300
show_descriptions = true
show_icons = true

[theme]
bg_color = 0x1e1e2e
fg_color = 0xcdd6f4
selected_bg = 0x89b4fa
selected_fg = 0x1e1e2e
border_color = 0x6c7086
query_bg = 0x313244
accent_color = 0xf38ba8
EOF
    
    print_success "Configuration file created at $CONFIG_PATH"
}

print_usage_info() {
    print_success "Installation complete!"
    echo
    print_status "Usage:"
    echo "  rufi                 # Launch the application launcher"
    echo "  man rufi             # View manual (if installed)"
    echo
    print_status "Configuration:"
    echo "  Edit ~/.rufirc to customize rufi"
    echo
    print_status "Window Manager Integration:"
    echo "  Add a key binding to launch rufi:"
    echo "    i3/sway: bindsym \$mod+d exec rufi"
    echo "    bspwm: super + d → rufi"
    echo "    awesome: awful.key({\$mod}, \"d\", function() awful.spawn(\"rufi\") end)"
    echo
    print_status "For more information, visit: https://github.com/yourusername/rufi"
}

main() {
    echo -e "${BLUE}"
    echo "╭────────────────────────────╮"
    echo "│     rufi Installation      │"
    echo "│   Fast X11 App Launcher    │"
    echo "╰────────────────────────────╯"
    echo -e "${NC}"
    
    check_dependencies
    install_rufi
    setup_config
    print_usage_info
}

# Handle command line arguments
case "${1:-}" in
    --help|-h)
        echo "rufi installation script"
        echo "Usage: $0 [OPTIONS]"
        echo ""
        echo "Options:"
        echo "  --help, -h     Show this help message"
        echo "  --config-only  Only install configuration file"
        exit 0
        ;;
    --config-only)
        setup_config
        exit 0
        ;;
    *)
        main
        ;;
esac
