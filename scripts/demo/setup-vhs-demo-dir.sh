#!/usr/bin/env bash

set -euo pipefail

DEMO_DIR="${1:-/tmp/lazyskills-vhs-demo}"

rm -rf "$DEMO_DIR"
mkdir -p "$DEMO_DIR"

echo "Prepared demo directory: $DEMO_DIR"
