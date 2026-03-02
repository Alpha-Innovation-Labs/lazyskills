# Bump patch version in Cargo.toml
# Usage: just pub-bump

pub-bump:
    #!/usr/bin/env bash
    set -euo pipefail

    CURRENT_VERSION=$(awk -F'"' '/^version = "[0-9]+\.[0-9]+\.[0-9]+"/ { print $2; exit }' Cargo.toml)
    if [[ -z "${CURRENT_VERSION:-}" ]]; then
        echo "Could not read current version from Cargo.toml"
        exit 1
    fi

    IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"
    NEW_VERSION="${MAJOR}.${MINOR}.$((PATCH + 1))"

    echo "Current version: $CURRENT_VERSION"
    echo "New version: $NEW_VERSION"

    awk -v v="$NEW_VERSION" '
        BEGIN { updated = 0 }
        {
            if (!updated && $0 ~ /^version = "[0-9]+\.[0-9]+\.[0-9]+"/) {
                sub(/^version = "[0-9]+\.[0-9]+\.[0-9]+"/, "version = \"" v "\"")
                updated = 1
            }
            print
        }
    ' Cargo.toml > Cargo.toml.tmp && mv Cargo.toml.tmp Cargo.toml

    cargo check --quiet

    echo "Updated Cargo.toml and Cargo.lock to $NEW_VERSION"
