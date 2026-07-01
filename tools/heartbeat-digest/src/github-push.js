/**
 * B3 Phase 3 — commit and push the regenerated digest files to GitHub.
 *
 * Disabled by default. Only touches files under `tools/heartbeat-digest/digests/`
 * so it never interferes with unrelated work-in-progress changes elsewhere in
 * the monorepo. Failure here is non-fatal: the Moultbook post (Phase 2) is the
 * canonical on-chain record, this step only helps the frontend/viewer catch up.
 */

import { execFileSync } from 'child_process'
import { dirname, join } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const HEARTBEAT_DIR = join(__dirname, '..') // tools/heartbeat-digest

const GIT_PUSH_ENABLED = process.env.GIT_PUSH_ENABLED === 'true'
const GIT_REMOTE = process.env.GIT_REMOTE || 'origin'

const DIGEST_PATHS = ['digests/latest.md', 'digests/latest-plain.md', 'digests/latest.json']

function git(args) {
  return execFileSync('git', args, { cwd: HEARTBEAT_DIR, encoding: 'utf8' }).trim()
}

export function isGitPushEnabled() {
  return GIT_PUSH_ENABLED
}

/**
 * Stage, commit, and push the digest files if (and only if) they actually
 * changed. Returns { pushed: boolean, commit: string | null }.
 */
export function pushDigest({ date, blockHeight, triggerReason }) {
  if (!GIT_PUSH_ENABLED) {
    return { pushed: false, commit: null, reason: 'disabled' }
  }

  const datedPath = `digests/${date}.md`
  const paths = [...DIGEST_PATHS, datedPath]

  try {
    git(['add', ...paths])

    const staged = git(['diff', '--cached', '--name-only', '--', ...paths])
    if (!staged) {
      return { pushed: false, commit: null, reason: 'no-changes' }
    }

    const heightPart = blockHeight != null ? `block ${blockHeight}` : 'digest update'
    const reasonPart = triggerReason ? ` — ${triggerReason}` : ''
    const message = `heartbeat: ${heightPart}${reasonPart}`

    git(['commit', '-m', message])
    const commitHash = git(['rev-parse', '--short', 'HEAD'])

    const branch = git(['rev-parse', '--abbrev-ref', 'HEAD'])
    git(['push', GIT_REMOTE, branch])

    return { pushed: true, commit: commitHash }
  } catch (err) {
    console.warn(`[github-push] failed: ${err.message}`)
    return { pushed: false, commit: null, reason: 'error', error: err.message }
  }
}
