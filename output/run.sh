#!/bin/bash

# Determine the OS and architecture
OS=$(uname -s | tr '[:upper:]' '[:lower:]')
ARCH=$(uname -m | tr '[:upper:]' '[:lower:]')

# Set the binary path based on OS and architecture
case "$OS" in
    linux)
        case "$ARCH" in
            x86_64)
                BINARY="bin/linux/collector-x86_64-unknown-linux-gnu"
                ;;
            aarch64)
                BINARY="bin/linux/collector-aarch64-unknown-linux-gnu"
                ;;
            *)
                exit_with_error "Unsupported architecture: $ARCH on Linux"
                ;;
        esac
        ;;
    darwin)
        case "$ARCH" in
            x86_64)
                BINARY="bin/macos/collector-x86_64-apple-darwin"
                ;;
            arm64)
                BINARY="bin/macos/collector-aarch64-apple-darwin"
                ;;
            *)
                exit_with_error "Unsupported architecture: $ARCH on macOS"
                ;;
        esac
        ;;
    cygwin*|mingw*|msys*)
        case "$ARCH" in
            x86_64)
                BINARY="bin/windows/collector-x86_64-pc-windows-msvc.exe"
                ;;
            aarch64)
                BINARY="bin/windows/collector-aarch64-pc-windows-msvc.exe"
                ;;
            *)
                exit_with_error "Unsupported architecture: $ARCH on Windows"
                ;;
        esac
        ;;
    *)
        exit_with_error "Unsupported OS: $OS"
        ;;
esac

# Function to handle unsupported architecture or OS
exit_with_error() {
    echo "$1"
    echo "Please manually run the binary in the bin directory"
    exit 1
}

# Run the binary
if [[ -x "$BINARY" ]]; then
    "$BINARY"
else
    echo "Binary not found or not executable: $BINARY"
    exit 1
fi
