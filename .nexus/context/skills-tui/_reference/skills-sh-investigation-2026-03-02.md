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

### 1.1) Homepage All-Time Leaderboard (UI-Parity Source)

- The homepage (`https://skills.sh/`) renders the visible "All Time" leaderboard directly in server-rendered page payload (HTML/RSC route behavior).
- Live verification showed the homepage leaderboard can diverge from `/api/skills/all-time/1` values at a given point in time.
- Example observed divergence:
  - Homepage top rows include `find-skills` (`~377K`) and related entries.
  - `/api/skills/all-time/1` returned a different ranking set (`~5.5K` installs for top rows in that sample).
- Practical implication:
  - If exact parity with what users see on skills.sh homepage is required, parse homepage leaderboard rows first.
  - Keep `/api/skills/all-time/{page}` as fallback when homepage parsing fails.

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
  1. For exact homepage parity in "All Time", parse leaderboard rows from `https://skills.sh/` payload.
  2. Use `/api/skills/{mode}/{page}` for fallback and large-scale list ingestion.
  3. Use `/<slug>.rsc` for detail enrichment.
  4. Keep HTML extraction as fallback for resilience.
- Risk:
  - RSC is a framework payload, not a guaranteed stable public schema.

## Supporting Verification Signals

- Site is built with Next.js and serves chunked assets under `/_next/static/chunks/`.
- Client-side chunk analysis showed `/api/search` and `/api/skills/*` usage.
- Homepage tab navigation also triggers route-level `?_rsc=...` requests (`/`, `/trending`, `/hot`) used to render visible leaderboard content.
- `vercel-labs/skills` CLI source (`src/find.ts`) references `https://skills.sh/api/search`.
