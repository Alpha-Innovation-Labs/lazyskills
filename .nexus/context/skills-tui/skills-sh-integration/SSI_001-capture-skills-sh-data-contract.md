---
context_id: SSI_001
title: Capture skills.sh data contract
project: skills-tui
feature: skills-sh-integration
created: "2026-03-02"
---

# SSI_001: Capture skills.sh data contract

## Desired Outcome

Skills TUI can reliably gather a complete skills catalog with install counts and enrich individual skills with detail fields from skills.sh using a documented, reproducible external contract that remains robust when one data surface changes.

## Reference

- Investigation report: `../_reference/skills-sh-investigation-2026-03-02.md`
- Confirmed list endpoint family: `/api/skills/{mode}/{page}`
- Confirmed search endpoint: `/api/search?q=<query>&limit=<n>`
- Confirmed detail surface: `/<owner>/<repo>/<skill>.rsc`

## Next Actions

| Description | Test |
|---|---|
| Define a normalized external contract for list records from `/api/skills/{mode}/{page}` including pagination behavior and required fields. | `list_contract_matches_live_payload` |
| Define a normalized external contract for search records from `/api/search` including required install and source fields. | `search_contract_matches_live_payload` |
| Define a normalized external contract for per-skill details from `/<slug>.rsc` covering weekly installs, repository, stars, first seen, audits, installed-on, and rendered content availability. | `detail_contract_matches_live_rsc_payload` |
| Define fallback behavior that uses skill page HTML extraction when RSC detail parsing fails or is unavailable. | `detail_fallback_uses_html_when_rsc_unavailable` |
| Validate that the combined ingestion flow can return complete list and detail data for at least one known skill slug end to end. | `end_to_end_catalog_and_detail_ingestion` |
