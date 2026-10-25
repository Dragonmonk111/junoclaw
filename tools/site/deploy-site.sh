#!/usr/bin/env bash
# Build + check + deploy junoclaw.xyz to Cloudflare Pages via wrangler.
# Folder upload keeps assets/, docs/, .well-known/ and _headers intact
# (zip drag-drop silently dropped directories three times).
#
#   tools/site/deploy-site.sh            -> preview deployment (branch "preview")
#   tools/site/deploy-site.sh --prod     -> production (branch "main")
#
# One-time: npx wrangler@4.151.1 login   (run it yourself; never paste tokens here)
# Project name: CF_PAGES_PROJECT=<name> (default: junoclaw)
set -euo pipefail
cd "$(dirname "$0")/../.."
PROJECT="${CF_PAGES_PROJECT:-junoclaw}"
BRANCH=preview
[ "${1:-}" = "--prod" ] && BRANCH=main

(cd docs-site && npm run build)
node tools/site/build-site.mjs
npx --yes wrangler@4.151.1 pages deploy website --project-name "$PROJECT" --branch "$BRANCH" --commit-dirty=true

if [ "$BRANCH" = main ]; then
  echo "--- post-deploy verification ---"
  for u in / /assets/logo-96.png /assets/fonts/fonts.css /docs/ /docs/lightpaper/ /docs/whats-new/ /.well-known/security.txt /robots.txt; do
    printf '%-32s %s\n' "$u" "$(curl -s -o /dev/null -w '%{http_code}' "https://junoclaw.xyz$u")"
  done
  printf '%-32s %s\n' "/no-such-page" "$(curl -s -o /dev/null -w '%{http_code}' https://junoclaw.xyz/no-such-page)  (expect 404)"
  curl -sI https://junoclaw.xyz/ | grep -iE '^(content-security-policy|strict-transport-security|x-frame-options|x-content-type-options):' || echo "WARNING: security headers missing"
fi
