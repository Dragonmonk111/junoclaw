import { readFileSync, existsSync, readdirSync } from 'fs'
import { join } from 'path'
import {
  DAO_CORE,
  PROPOSAL_MODULE,
  MOTHER_MOULT_ID,
  REPLY_BOT_NAME,
  DAO_DAO_BASE,
  OVERRIDE_DIR,
  DEFAULT_TAGS,
  DEFAULT_REFS,
} from './config.js'

function formatExpiration(exp) {
  if (!exp) return 'unknown'
  if (exp.at_time) return new Date(Number(exp.at_time) / 1_000_000).toISOString()
  if (exp.at_height) return `height ${exp.at_height}`
  return JSON.stringify(exp)
}

function buildDaoUrl(id) {
  return `${DAO_DAO_BASE}/dao/${DAO_CORE}/proposals/${id}`
}

function normalizeThreshold(threshold) {
  if (!threshold) return null
  const pct = threshold.absolute_percentage?.percentage
  if (pct) return `${Number(pct) * 100}%`
  return JSON.stringify(threshold)
}

function findOverrideFile(id) {
  try {
    if (!existsSync(OVERRIDE_DIR)) return null
    const files = readdirSync(OVERRIDE_DIR)
    const match = files.find((f) => f.toLowerCase().startsWith(`a${id}_`) && f.endsWith('.md'))
    return match ? join(OVERRIDE_DIR, match) : null
  } catch {
    return null
  }
}

export function buildAkbEnvelope({ id, body, tags = [], extraRefs = [] }) {
  const url = buildDaoUrl(id)
  const expiresAt = formatExpiration(body.expiration)
  const threshold = normalizeThreshold(body.threshold)
  const votes = body.votes || {}
  const totalVotes = Number(votes.yes || 0) + Number(votes.no || 0) + Number(votes.abstain || 0)

  const overrideFile = findOverrideFile(id)
  const overrideText = overrideFile ? readFileSync(overrideFile, 'utf8') : null
  const descriptionText = overrideText || body.description || ''
  const summary = descriptionText.length > 4000 ? descriptionText.slice(0, 4000) + '\n\n[truncated]' : descriptionText

  const text = `# A${id} — ${body.title || '(untitled)'}

- **Status:** ${body.status}
- **Proposer:** ${body.proposer}
- **Total voting power:** ${body.total_power}
- **Expires:** ${expiresAt}
- **Threshold:** ${threshold || 'not set'}
- **Votes:** yes ${votes.yes || 0}, no ${votes.no || 0}, abstain ${votes.abstain || 0} (total ${totalVotes})
- **DAO DAO link:** ${url}

## Description

${summary}`

  const defaultTags = ['dao-proposal', `A${id}`, body.status]
  const finalTags = [...new Set([...DEFAULT_TAGS, ...defaultTags, ...tags])]

  const refs = [...DEFAULT_REFS]
  for (const r of extraRefs) {
    const ref = r.startsWith('moult:') ? r : `moult:${r}`
    if (!refs.includes(ref)) refs.push(ref)
  }

  return {
    akb_version: '1.1',
    direction: 'export',
    mother_moult_id: MOTHER_MOULT_ID,
    author: {
      wallet: process.env.REPLY_BOT_WALLET || 'juno1unknownplaceholder',
      alias: REPLY_BOT_NAME,
      type: 'agent',
    },
    content: {
      mime_type: 'application/json+agent-proposal',
      text,
      structured: {
        dao_core: DAO_CORE,
        proposal_module: PROPOSAL_MODULE,
        proposal_id: id,
        title: body.title,
        status: body.status,
        proposer: body.proposer,
        total_power: body.total_power,
        expiration: expiresAt,
        threshold: body.threshold,
        votes,
        total_votes: totalVotes,
        funds: body.funds || [],
        msgs: body.msgs || [],
        url,
        source: 'on-chain DAO DAO proposal',
        override_file: overrideFile || null,
      },
    },
    refs,
    tags: finalTags,
  }
}
