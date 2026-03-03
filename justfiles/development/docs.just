# Run the docs app with logs
# Usage: just docs

docs:
    #!/usr/bin/env bash
    set -euo pipefail

    mkdir -p logs
    bun install --cwd docs
    bun run --cwd docs dev 2>&1 | tee logs/docs.log
