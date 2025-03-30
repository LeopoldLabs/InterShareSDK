# Navigate to the src/data_rct_ffi directory
Push-Location src/intershare_sdk

# Build for x86_64-pc-windows-msvc
cargo build --lib --release --target x86_64-pc-windows-msvc
# Build for aarch64-pc-windows-msvc
cargo build --lib --release --target aarch64-pc-windows-msvc
# Build for aarch64-pc-windows-msvc
# cargo build --lib --release --features sync --target aarch64-pc-windows-msvc

# Return to the previous directory
Pop-Location

# Run uniffi-bindgen for C# bindings generation
cargo build --release
uniffi-bindgen-cs --library --out-dir="bindings/csharp/InterShareSdk" .\target\release\intershare_sdk.dll
