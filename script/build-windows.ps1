# Install dependencies
choco install strawberryperl
choco install openssl
choco install llvm

# Set up Rust toolchain and target
rustup target add x86_64-pc-windows-msvc

# Build project
$env:RUSTFLAGS='-C target-feature=+crt-static'
cargo build --release --target x86_64-pc-windows-msvc
