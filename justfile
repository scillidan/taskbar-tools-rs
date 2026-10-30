default: test

build:
    cargo build --release

test:
    cargo test --workspace

check:
    cargo fmt --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

dist: build
    powershell -NoProfile -ExecutionPolicy Bypass -File package.ps1
