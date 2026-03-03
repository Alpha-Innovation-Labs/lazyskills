# lazyskills

`lazyskills` is a Rust terminal UI for browsing, previewing, and managing AI coding skills.

## Install

| Ecosystem | Install Command |
|-----------|------------------|
| Cargo (crates.io) | `cargo install lazyskills --bin lazyskills` |
| npm | `npm install -g lazyskills` |
| uv / PyPI | `uv tool install lazyskills` |
| Homebrew | `brew tap Alpha-Innovation-Labs/tap && brew install lazyskills` |
| Scoop | `scoop bucket add alpha-innovation-labs https://github.com/Alpha-Innovation-Labs/scoop-bucket && scoop install lazyskills` |

## Local development

```bash
just dev
```

## Demo: VHS

Run the demo recipe:

```bash
just demo-vhs
```

Current tape scope: it opens `lazyskills` and explicitly navigates the initial default-agent modal (selects `claude-code` and saves). The search/install/favorite steps are intentionally commented out for now.

## Release artifacts

Release assets are expected to use these names:

- `lazyskills-aarch64-apple-darwin`
- `lazyskills-x86_64-unknown-linux-gnu`
- `lazyskills-x86_64-pc-windows-msvc.exe`
