# Build and publish docs to gh-pages
# Usage: just docs-gh-pub

docs-gh-pub:
    #!/usr/bin/env bash
    set -euo pipefail

    if ! command -v bun >/dev/null 2>&1; then
        echo "bun is required for docs publication"
        exit 1
    fi

    if ! git remote get-url origin >/dev/null 2>&1; then
        echo "origin remote is required for docs publication"
        exit 1
    fi

    REMOTE_URL="$(git remote get-url origin)"
    REPO_NAME="$(basename "${REMOTE_URL%.git}")"

    echo "Publishing docs for repository: ${REPO_NAME}"

    bun --cwd docs install
    DOCS_GH_PAGES=1 DOCS_REPO_NAME="${REPO_NAME}" bun --cwd docs run build
    touch docs/out/.nojekyll

    TMP_WORKTREE="$(mktemp -d)"

    cleanup() {
        git worktree remove --force "${TMP_WORKTREE}" >/dev/null 2>&1 || true
        rm -rf "${TMP_WORKTREE}" >/dev/null 2>&1 || true
    }
    trap cleanup EXIT

    if git show-ref --verify --quiet refs/heads/gh-pages; then
        git worktree add "${TMP_WORKTREE}" gh-pages
    elif git ls-remote --exit-code --heads origin gh-pages >/dev/null 2>&1; then
        git fetch origin gh-pages:gh-pages
        git worktree add "${TMP_WORKTREE}" gh-pages
    else
        git worktree add -b gh-pages "${TMP_WORKTREE}" HEAD
        git -C "${TMP_WORKTREE}" rm -rf . >/dev/null 2>&1 || true
    fi

    rsync -a --delete --exclude='.git' docs/out/ "${TMP_WORKTREE}/"

    if [[ -z "$(git -C "${TMP_WORKTREE}" status --porcelain)" ]]; then
        echo "No documentation changes detected; nothing to publish"
        exit 0
    fi

    git -C "${TMP_WORKTREE}" add -A
    git -C "${TMP_WORKTREE}" commit -m "docs: publish static site"
    git -C "${TMP_WORKTREE}" push origin gh-pages

    echo "Published docs to gh-pages"
