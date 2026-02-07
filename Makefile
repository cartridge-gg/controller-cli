.PHONY: help install build release clean test lint fmt check

# Default target
help:
	@echo "Cartridge Controller CLI - Make targets:"
	@echo ""
	@echo "  install     - Install the CLI binary to ~/.local/bin"
	@echo "  build       - Build debug binary"
	@echo "  release     - Build optimized release binary"
	@echo "  clean       - Remove build artifacts"
	@echo "  test        - Run tests"
	@echo "  lint        - Run clippy linter"
	@echo "  fmt         - Format code"
	@echo "  check       - Run all checks (fmt + lint + test)"
	@echo ""

# Install to ~/.local/bin
install: release
	@mkdir -p ~/.local/bin
	@cp target/release/controller-cli ~/.local/bin/
	@echo "✅ Installed to ~/.local/bin/controller-cli"
	@echo ""
	@echo "Make sure ~/.local/bin is in your PATH:"
	@echo "  export PATH=\"\$$PATH:~/.local/bin\""

# Build debug binary
build:
	cargo build

# Build release binary
release:
	cargo build --release

# Clean build artifacts
clean:
	cargo clean

# Run tests
test:
	cargo test

# Run clippy
lint:
	cargo clippy -- -D warnings

# Format code
fmt:
	cargo fmt

# Run all checks
check: fmt lint test
	@echo "✅ All checks passed"

# Run the CLI
run:
	cargo run --

# Generate keypair (example)
example-keygen:
	cargo run -- generate-keypair

# Check status (example)
example-status:
	cargo run -- status --json
