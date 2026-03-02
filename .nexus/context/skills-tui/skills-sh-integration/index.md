---
project_id: skills-sh-integration
title: Skills.sh Integration
created: "2026-03-02"
status: active
dependencies: []
---

# Scope

Track the external data contracts required to retrieve skills list/install metrics and per-skill detail data from skills.sh.

# Context Files

| Context ID | File | Outcome |
|---|---|---|
| SSI_001 | `SSI_001-capture-skills-sh-data-contract.md` | Lock a reproducible contract for list and detail ingestion from skills.sh. |

# Interfaces

- List/search JSON endpoints under `https://skills.sh/api/...`
- Skill detail data via Next.js RSC payload at `https://skills.sh/<owner>/<repo>/<skill>.rsc`

# Dependencies

- No blocking feature dependencies documented.

# Troubleshooting

- If `.rsc` payload shape changes, fall back to HTML extraction while updating the parser contract.
