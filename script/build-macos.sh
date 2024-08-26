#!/bin/bash

# Install dependencies
brew install openssl

# Set up Rust toolchain and target
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Build project
RUSTFLAGS='-C target-feature=+crt-static -lcrypto -lssl' cargo build --release --target x86_64-apple-darwin
RUSTFLAGS='-C target-feature=+crt-static -lcrypto -lssl' cargo build --release --target aarch64-apple-darwin
