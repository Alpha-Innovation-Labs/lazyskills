# Generate docs reference from Rust source comments
# Usage: just docs-sync

docs-sync:
    cargo run --bin docs-sync -- sync
