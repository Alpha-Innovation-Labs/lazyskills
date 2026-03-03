# Prepare demo workspace and render the VHS demo GIF
# Usage: just demo-vhs

demo-vhs:
	#!/usr/bin/env bash
	set -euo pipefail

	DEMO_DIR="/tmp/lazyskills-vhs-demo"
	REPO_DIR="$(pwd)"
	DEMO_GIF="$REPO_DIR/demos/lazyskills-demo.gif"
	DOCS_GIF="$REPO_DIR/docs/public/media/lazyskills-demo.gif"

	cargo build --quiet --bin lazyskills
	"$REPO_DIR/scripts/demo/setup-vhs-demo-dir.sh" "$DEMO_DIR"
	cd "$DEMO_DIR"
	PATH="$REPO_DIR/target/debug:$PATH" XDG_DATA_HOME="$DEMO_DIR/.demo-xdg" vhs "$REPO_DIR/demos/lazyskills-find-skills.tape" -o "$DEMO_GIF"
	mkdir -p "$REPO_DIR/docs/public/media"
	cp "$DEMO_GIF" "$DOCS_GIF"
