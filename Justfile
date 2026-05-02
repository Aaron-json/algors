# Run Loom model checks for the queue crate
test-loom:
    RUSTFLAGS="--cfg loom" cargo test -p algors-queue --release

test:
    cargo test --workspace

# Check all crates for compilation errors across all feature sets
check:
    cargo check --workspace --all-features
    RUSTFLAGS="--cfg loom" cargo check --workspace --all-features
