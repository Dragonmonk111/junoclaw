import { existsSync, readFileSync, writeFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const STATE_FILE = process.env.INDEXER_STATE_FILE || join(__dirname, '..', 'indexer-state.json')

const defaultState = {
  last_seen_height: 0,
  posted: [],
}

export function loadState() {
  if (!existsSync(STATE_FILE)) return defaultState
  try {
    const raw = readFileSync(STATE_FILE, 'utf8')
    const parsed = JSON.parse(raw)
    return {
      ...defaultState,
      ...parsed,
      posted: Array.isArray(parsed.posted) ? parsed.posted : defaultState.posted,
    }
  } catch (e) {
    console.warn('[indexer-state] could not parse state file, starting fresh:', e.message)
    return defaultState
  }
}

export function saveState(state) {
  writeFileSync(STATE_FILE, JSON.stringify(state, null, 2))
}

export function isPosted(state, proposalId) {
  return state.posted.some((p) => String(p.proposal_id) === String(proposalId))
}

export function markPosted(state, record) {
  state.posted = state.posted.filter((p) => String(p.proposal_id) !== String(record.proposal_id))
  state.posted.push(record)
  return state
}
