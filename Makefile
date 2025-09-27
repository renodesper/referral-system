.PHONY: build release clean fmt check test install doc help tasks

default: help

# Default target (ensures formatting before building)
build: fmt
	cargo build --release

# Full release process (ensures everything runs in the correct order)
release: fmt check build test install doc

# Format the code
fmt:
	cargo fmt

# Check for errors without building
check:
	cargo check

# Strict linter, fails on warning and suggests fixes
clippy:
	cargo clippy -- -D warnings

# Run tests
test:
	cargo test

# Install the binary
install:
	cargo install --path .

# Generate documentation
doc:
	cargo doc

# Publish to crates.io
publish:
	cargo publish

# Clean build artifacts
clean:
	cargo clean

# Show all available tasks
help tasks:
	@echo "Available commands:"
	@grep -E '^[a-zA-Z_-]+:.*##' $(MAKEFILE_LIST) | awk 'BEGIN {FS = ":.*## "}; {printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}'
