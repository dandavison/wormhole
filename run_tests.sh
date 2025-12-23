#!/bin/bash

# Build the project first
echo "Building wormhole..."
cargo build --quiet || exit 1

# Check if there's a wormhole process already running on the test ports
for port in 7777 7778 7779 7780 7781 7782 7783 7784; do
    if lsof -i:$port >/dev/null 2>&1; then
        echo "Error: Port $port is already in use. Please stop any test servers."
        exit 1
    fi
done

# Run the tests
echo "Running tests..."
cargo test -- --test-threads=1 --nocapture

# Clean up any leftover capture files
rm -f /tmp/wormhole_test_capture_*.json

echo "Tests complete."
