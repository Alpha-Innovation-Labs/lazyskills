# skills.sh Investigation Report (2026-03-02)

## Objective

Identify how skills.sh serves:
- Full skills list with install counts
- Per-skill detail fields shown on skill pages

## Confirmed Data Surfaces

### 1) Full List and Installs

- Ranked/paginated list endpoint:
  - `https://skills.sh/api/skills/all-time/{page}`
  - Also observed: `trending` and `hot` modes
- Response fields observed:
  - `skills[]` entries containing fields such as `id`, `skillId`, `name`, `source`, `installs`
  - `total`, `hasMore`, `page`
- Pagination details observed:
  - `pageSize = 200`
  - `total = 82056` on page 1 sample

### 2) Search with Installs

- Search endpoint:
  - `https://skills.sh/api/search?q=<query>&limit=<n>`
- Response includes:
  - `skills[]` with `id`, `skillId`, `name`, `source`, `installs`
  - top-level `count`

### 3) Skill Detail Page Data

- No stable public JSON detail endpoint was confirmed for `/<owner>/<repo>/<skill>`.
- Detail fields are exposed in Next.js RSC payload:
  - `https://skills.sh/<owner>/<repo>/<skill>.rsc`
  - Alternative route style also observed via page URL with `?_rsc=...` and RSC headers.
- RSC payload contains values used by skill page UI, including:
  - Weekly installs
  - Repository
  - GitHub stars
  - First seen
  - Security audits
  - Installed-on breakdown
  - Rendered `SKILL.md` content

## Integration Notes

- Preferred ingestion strategy:
  1. Use `/api/skills/{mode}/{page}` for large-scale list ingestion.
  2. Use `/<slug>.rsc` for detail enrichment.
  3. Keep HTML extraction as fallback for resilience.
- Risk:
  - RSC is a framework payload, not a guaranteed stable public schema.

## Supporting Verification Signals

- Site is built with Next.js and serves chunked assets under `/_next/static/chunks/`.
- Client-side chunk analysis showed `/api/search` and `/api/skills/*` usage.
- `vercel-labs/skills` CLI source (`src/find.ts`) references `https://skills.sh/api/search`.
