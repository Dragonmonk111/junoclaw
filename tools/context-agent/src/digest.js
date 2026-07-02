import { readFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'
import { getEntry } from './moultbook.js'

const __dirname = dirname(fileURLToPath(import.meta.url))
const DIGESTS_DIR = process.env.DIGESTS_DIR || join(__dirname, '..', '..', 'heartbeat-digest', 'digests')

export async function getLatestDigestContent(prefer = 'github') {
  if (prefer === 'github') {
    try {
      return readFileSync(join(DIGESTS_DIR, 'latest.md'), 'utf8')
    } catch (e) {
      // fall through to on-chain
    }
  }

  // On-chain fallback: fetch the latest entry, then we would need a way to
  // decode the commitment. For now, Moultbook stores the SHA-256 commitment,
  // not the raw markdown, so the GitHub mirror is the practical source.
  throw new Error('Latest digest not available from GitHub mirror')
}

export async function getLatestDigestJson() {
  try {
    return JSON.parse(readFileSync(join(DIGESTS_DIR, 'latest.json'), 'utf8'))
  } catch (e) {
    return null
  }
}
