#!/bin/bash

# Install dependencies
sudo apt-get update
sudo apt-get install -y pkg-config libssl-dev

# Set up Rust toolchain and target
rustup target add x86_64-unknown-linux-gnu

# Build project
RUSTFLAGS='-C target-feature=+crt-static -lcrypto -lssl' cargo build --release --target x86_64-unknown-linux-gnu
