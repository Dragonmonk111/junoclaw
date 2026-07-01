/**
 * B3 Phase 1 — polled watcher with state diff.
 *
 * Polls the DAO's on-chain state on an interval, hashes the parts that
 * matter (proposals, votes, members, treasury), and only regenerates the
 * heartbeat digest when that hash actually changes. No Moultbook posting
 * yet — see PLAN_B3_BLOCK_DRIVEN_HEARTBEAT.md for Phase 2+.
 *
 * Run once and exit:   RUN_ONCE=true node src/watch.js
 * Run continuously:    node src/watch.js   (Ctrl+C to stop)
 */

import { writeFileSync, existsSync, mkdirSync, readFileSync } from 'fs'
import { dirname, join } from 'path'
import { fileURLToPath } from 'url'
import { createHash } from 'crypto'
import {
  DAO_CORE,
  PROPOSAL_MODULE,
  REST_ENDPOINT,
  getVotingModule,
  getProposals,
  getMembers,
  getTreasury,
  buildDigestData,
} from './index.js'
import { renderDigest } from './render-rich.js'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

// ── Configuration ───────────────────────────────────────────────────────────

const DIGESTS_DIR = join(__dirname, '..', 'digests')
const STATE_DIR = join(__dirname, '..', 'state')
const STATE_PATH = join(STATE_DIR, 'last-state.json')

const POLL_INTERVAL_MS = Number(process.env.POLL_INTERVAL_MS || 5 * 60 * 1000) // 5 minutes
const RUN_ONCE = process.env.RUN_ONCE === 'true'

// ── Helpers ─────────────────────────────────────────────────────────────────

function todayISO() {
  return new Date().toISOString().split('T')[0]
}

async function getBlockHeight() {
  const url = `${REST_ENDPOINT}/cosmos/base/tendermint/v1beta1/blocks/latest`
  const res = await fetch(url)
  if (!res.ok) {
    throw new Error(`block height query failed ${res.status}`)
  }
  const json = await res.json()
  return Number(json.block?.header?.height || 0)
}

// ── Canonical state — only the fields that matter for "did anything change" ─

function canonicalState({ proposals, members, treasury }) {
  const props = proposals
    .map((p) => {
      const body = p.proposal || p
      const votes = body.votes || {}
      return {
        id: p.id,
        status: body.status,
        yes: Number(votes.yes || 0),
        no: Number(votes.no || 0),
        abstain: Number(votes.abstain || 0),
      }
    })
    .sort((a, b) => a.id - b.id)

  const mems = members
    .map((m) => ({ addr: m.addr, weight: Number(m.weight || 0), role: m.role || null }))
    .sort((a, b) => a.addr.localeCompare(b.addr))

  const treas = treasury
    .map((t) => ({ denom: t.denom, amount: t.amount }))
    .sort((a, b) => a.denom.localeCompare(b.denom))

  return { proposals: props, members: mems, treasury: treas }
}

function hashState(state) {
  return createHash('sha256').update(JSON.stringify(state)).digest('hex')
}

// ── Diffing for human-readable changes (feeds meta.changes / trigger_reason) ─

function diffState(prev, next) {
  const changes = []
  const reasons = new Set()

  if (!prev) {
    return { changes: ['Initial state snapshot'], trigger_reason: 'initial' }
  }

  const prevProps = new Map(prev.proposals.map((p) => [p.id, p]))
  for (const p of next.proposals) {
    const before = prevProps.get(p.id)
    if (!before) {
      changes.push(`Proposal A${p.id} created (status: ${p.status})`)
      reasons.add('proposal_created')
      continue
    }
    if (before.status !== p.status) {
      changes.push(`Proposal A${p.id} status changed: ${before.status} → ${p.status}`)
      reasons.add('proposal_status_changed')
    } else if (before.yes !== p.yes || before.no !== p.no || before.abstain !== p.abstain) {
      changes.push(
        `Vote cast on A${p.id} (yes ${before.yes}→${p.yes}, no ${before.no}→${p.no}, abstain ${before.abstain}→${p.abstain})`,
      )
      reasons.add('vote_cast')
    }
  }

  if (JSON.stringify(prev.members) !== JSON.stringify(next.members)) {
    changes.push('Membership or voting weight changed')
    reasons.add('membership_change')
  }

  const prevTreasury = new Map(prev.treasury.map((t) => [t.denom, t.amount]))
  for (const t of next.treasury) {
    const before = prevTreasury.get(t.denom)
    if (before !== t.amount) {
      changes.push(`Treasury ${t.denom} changed: ${before ?? '0'} → ${t.amount}`)
      reasons.add('treasury_change')
    }
  }

  if (changes.length === 0) {
    changes.push('State hash changed but no tracked field differs')
    reasons.add('state_changed')
  }

  return { changes, trigger_reason: [...reasons].join(',') }
}

// ── State persistence ─────────────────────────────────────────────────────────

function loadLastState() {
  if (!existsSync(STATE_PATH)) return null
  try {
    return JSON.parse(readFileSync(STATE_PATH, 'utf8'))
  } catch (e) {
    console.warn('[watch] could not parse last-state.json, treating as first run:', e.message)
    return null
  }
}

function saveState(state) {
  if (!existsSync(STATE_DIR)) mkdirSync(STATE_DIR, { recursive: true })
  writeFileSync(STATE_PATH, JSON.stringify(state, null, 2), 'utf8')
}

// ── One watch cycle ───────────────────────────────────────────────────────────

async function runOnce() {
  const lastState = loadLastState()

  const [blockHeight, votingModule] = await Promise.all([getBlockHeight(), getVotingModule()])
  const [proposals, members, treasury] = await Promise.all([
    getProposals(),
    getMembers(votingModule),
    getTreasury(),
  ])

  const state = canonicalState({ proposals, members, treasury })
  const stateHash = hashState(state)

  if (lastState && lastState.state_hash === stateHash) {
    console.log(`[watch] block ${blockHeight}: no meaningful change (hash ${stateHash.slice(0, 12)}...)`)
    return
  }

  const { changes, trigger_reason } = diffState(lastState?.state || null, state)
  console.log(`[watch] block ${blockHeight}: change detected — ${trigger_reason}`)
  changes.forEach((c) => console.log(`  - ${c}`))

  const date = todayISO()
  const data = buildDigestData({ date, proposals, members, treasury })
  data.meta.block_height = blockHeight
  data.meta.trigger_reason = trigger_reason
  data.meta.changes = changes
  data.meta.previous_moultbook = lastState?.last_digest_moult_id || null

  const { plain, rich } = renderDigest(data)

  if (!existsSync(DIGESTS_DIR)) mkdirSync(DIGESTS_DIR, { recursive: true })
  writeFileSync(join(DIGESTS_DIR, 'latest.md'), rich, 'utf8')
  writeFileSync(join(DIGESTS_DIR, `${date}.md`), rich, 'utf8')
  writeFileSync(join(DIGESTS_DIR, 'latest-plain.md'), plain, 'utf8')
  writeFileSync(join(DIGESTS_DIR, 'latest.json'), JSON.stringify(data, null, 2), 'utf8')
  console.log(`[watch] digest regenerated → ${join(DIGESTS_DIR, 'latest.json')}`)

  saveState({
    block_height: blockHeight,
    state_hash: stateHash,
    state,
    trigger_reason,
    // Phase 1 does not post to Moultbook yet — carry the previous id forward untouched.
    last_digest_moult_id: lastState?.last_digest_moult_id || null,
    last_generated_at: new Date().toISOString(),
  })
}

async function main() {
  console.log(`[watch] Heartbeat watcher starting for ${DAO_CORE}`)
  console.log(`[watch] Proposal module: ${PROPOSAL_MODULE}`)
  console.log(`[watch] REST endpoint: ${REST_ENDPOINT}`)

  await runOnce().catch((err) => console.error('[watch] cycle failed:', err.message))

  if (RUN_ONCE) {
    console.log('[watch] RUN_ONCE=true, exiting after one cycle.')
    return
  }

  console.log(`[watch] polling every ${POLL_INTERVAL_MS / 1000}s. Press Ctrl+C to stop.`)
  setInterval(() => {
    runOnce().catch((err) => console.error('[watch] cycle failed:', err.message))
  }, POLL_INTERVAL_MS)
}

main()
