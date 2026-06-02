#!/bin/sh

# integration-test.sh - Script to run integration tests for Cloudflare Worker
# Usage: integration-test.sh

echo "Checking if wrangler is installed..."
if ! command -v wrangler >/dev/null 2>&1; then
    echo "Error: wrangler is not installed"
    echo "Please install it with: npm install -g wrangler"
    exit 1
fi

echo "Running cargo tests as baseline..."
if cargo test; then
    echo "Cargo tests passed"
else
    echo "Cargo tests failed"
    exit 1
fi

echo "Note: Full integration tests require 'wrangler dev' to be running"
echo "To run full integration tests:"
echo "1. Start the development server: task dev"
echo "2. In another terminal, run: task integration-test"
exit 0