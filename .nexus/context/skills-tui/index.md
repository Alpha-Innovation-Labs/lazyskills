---
project_id: skills-tui
title: Skills TUI
created: "2026-03-02"
status: active
dependencies: []
---

# Overview

Skills TUI is a Rust-based terminal application for skills workflows and related tooling in this repository.

# Features

| Feature | Purpose | Status |
|---|---|---|
| skills-sh-integration | Define and track external data contracts from skills.sh for list and detail views. | active |

# Architecture

```text
skills-tui
|-- local app/runtime
|-- external data sources
|   `-- skills.sh (public web + APIs)
`-- context docs (.nexus/context/skills-tui/...)
```

# Design References

- `./design_references/design_reference.md`

# Operational Notes

- Use context files under feature folders for outcome-level work.
- Keep third-party endpoint discoveries in `_reference` for traceability.
