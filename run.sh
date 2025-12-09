#!/bin/bash

# Showel - PostgreSQL Database Manager
# Development run script

set -e

echo "🚀 Starting Showel..."
echo ""

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    echo "❌ Error: Cargo is not installed"
    echo "Please install Rust from https://rustup.rs/"
    exit 1
fi

# Check for PostgreSQL
if ! command -v psql &> /dev/null; then
    echo "⚠️  Warning: psql not found in PATH"
    echo "PostgreSQL may not be installed"
    echo ""
fi

# Build and run
echo "Building Showel..."
cargo build --release

if [ $? -eq 0 ]; then
    echo ""
    echo "✅ Build successful!"
    echo "Starting application..."
    echo ""
    cargo run --release
else
    echo ""
    echo "❌ Build failed"
    exit 1
fi
