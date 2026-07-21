import { readFileSync } from 'fs'
import { postAkbExportToMoultbook } from '../src/moultbook.js'

// ── Configuration ───────────────────────────────────────────────────────────

const DAO_CORE = process.env.DAO_CORE || 'juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac'
const PROPOSAL_MODULE = process.env.PROPOSAL_MODULE || 'juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp'
const REST_ENDPOINT = process.env.REST_ENDPOINT || 'https://juno-rest.publicnode.com'
const MOTHER_MOULT_ID = process.env.MOTHER_MOULT_ID || 'moult:49b0b8f5ee0832959920f6432bd6c15cc7551c07c48980a6efb0b28e657c7e2a'
const REPLY_BOT_NAME = process.env.REPLY_BOT_NAME || 'dragonmonk111-bot'
const DAO_DAO_BASE = process.env.DAO_DAO_BASE || 'https://dao.daodao.zone'
const REPLY_BOT_SERVER_URL = process.env.REPLY_BOT_SERVER_URL || 'http://localhost:3001'
const REPLY_BOT_ADMIN_TOKEN = process.env.REPLY_BOT_ADMIN_TOKEN || ''

// ── CLI args ──────────────────────────────────────────────────────────────────

function parseArgs() {
  const args = process.argv.slice(2)
  let proposalId = null
  let file = null
  let tags = []
  let refs = []
  let useServer = false
  let preview = false

  for (let i = 0; i < args.length; i++) {
    const arg = args[i]
    if (arg === '--proposal-id' || arg === '-id') {
      proposalId = Number(args[++i])
    } else if (arg.startsWith('--proposal-id=')) {
      proposalId = Number(arg.split('=')[1])
    } else if (arg === '--file' || arg === '-f') {
      file = args[++i]
    } else if (arg.startsWith('--file=')) {
      file = arg.split('=')[1]
    } else if (arg === '--tags') {
      tags = args[++i]?.split(',').map((t) => t.trim()).filter(Boolean) || []
    } else if (arg.startsWith('--tags=')) {
      tags = arg.split('=')[1]?.split(',').map((t) => t.trim()).filter(Boolean) || []
    } else if (arg === '--refs') {
      refs = args[++i]?.split(',').map((r) => r.trim()).filter(Boolean) || []
    } else if (arg.startsWith('--refs=')) {
      refs = arg.split('=')[1]?.split(',').map((r) => r.trim()).filter(Boolean) || []
    } else if (arg === '--use-server' || arg === '-s') {
      useServer = true
    } else if (arg === '--preview' || arg === '-p') {
      preview = true
    } else if (arg === '--help' || arg === '-h') {
      printUsage()
      process.exit(0)
    }
  }

  return { proposalId, file, tags, refs, useServer, preview }
}

function printUsage() {
  console.log(`Usage: node post-dao-proposal.js --proposal-id <id> [options]

Options:
  --proposal-id, -id    DAO DAO proposal id (required)
  --file, -f            Local markdown/txt file to use as description override
  --tags                Comma-separated tags (default: dao-proposal, A{id})
  --refs                Comma-separated moult/proposal ids to cite
  --use-server, -s      POST through the running reply-bot server instead of signing locally
  --preview, -p         When --use-server is set, only preview the draft (approve=false)

Env:
  DAO_CORE, PROPOSAL_MODULE, REST_ENDPOINT, MOTHER_MOULT_ID,
  REPLY_BOT_NAME, JUNO_REPLY_BOT_MNEMONIC, MOULTBOOK_DRY_RUN=true,
  REPLY_BOT_SERVER_URL, REPLY_BOT_ADMIN_TOKEN
`)
}

// ── Chain helpers ─────────────────────────────────────────────────────────────

async function querySmart(contract, msg) {
  const query = Buffer.from(JSON.stringify(msg)).toString('base64')
  const url = `${REST_ENDPOINT}/cosmwasm/wasm/v1/contract/${contract}/smart/${query}`
  const res = await fetch(url)
  if (!res.ok) {
    const text = await res.text()
    throw new Error(`query failed ${res.status}: ${text}`)
  }
  const json = await res.json()
  return json.data ?? json
}

async function fetchProposal(id) {
  const resp = await querySmart(PROPOSAL_MODULE, { proposal: { proposal_id: id } })
  // DAO DAO proposal modules wrap the body under a `proposal` key.
  return resp.proposal || resp
}

// ── Formatting ────────────────────────────────────────────────────────────────

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
  if (pct) {
    // DAO DAO stores percentage as decimal string: "0.5" = 50%
    return `${Number(pct) * 100}%`
  }
  return JSON.stringify(threshold)
}

// ── Server-side poster ────────────────────────────────────────────────────────

async function postViaServer(envelope, preview = false) {
  const url = `${REPLY_BOT_SERVER_URL}/api/export`
  const headers = { 'Content-Type': 'application/json' }
  if (REPLY_BOT_ADMIN_TOKEN) {
    headers['Authorization'] = `Bearer ${REPLY_BOT_ADMIN_TOKEN}`
  }

  const res = await fetch(url, {
    method: 'POST',
    headers,
    body: JSON.stringify({ envelope, approve: !preview }),
  })

  const text = await res.text()
  let json
  try {
    json = JSON.parse(text)
  } catch {
    throw new Error(`server returned non-JSON ${res.status}: ${text.slice(0, 500)}`)
  }

  if (!res.ok) {
    throw new Error(`server returned ${res.status}: ${json.error || text}`)
  }

  return json
}

// ── AKB export builder ────────────────────────────────────────────────────────

function buildAkbEnvelope({ id, body, overrideText, tags, extraRefs }) {
  const url = buildDaoUrl(id)
  const expiresAt = formatExpiration(body.expiration)
  const threshold = normalizeThreshold(body.threshold)
  const votes = body.votes || {}
  const totalVotes = Number(votes.yes || 0) + Number(votes.no || 0) + Number(votes.abstain || 0)

  const chainDescription = body.description || ''
  const descriptionText = overrideText || chainDescription
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

  const defaultTags = ['dao-proposal', `A${id}`]
  const finalTags = [...new Set([...defaultTags, ...tags])]

  const refs = [MOTHER_MOULT_ID]
  for (const r of extraRefs) {
    const ref = r.startsWith('moult:') ? r : `moult:${r}`
    if (!refs.includes(ref)) refs.push(ref)
  }

  return {
    akb_version: '1.1',
    direction: 'export',
    mother_moult_id: MOTHER_MOULT_ID,
    author: {
      wallet: process.env.REPLY_BOT_WALLET || (process.env.MOULTBOOK_DRY_RUN === 'true' ? 'juno1dryrun000000000000000000000000000000000' : ''),
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
      },
    },
    refs,
    tags: finalTags,
  }
}

// ── Main ────────────────────────────────────────────────────────────────────────

async function main() {
  const { proposalId, file, tags, refs, useServer, preview } = parseArgs()

  if (!proposalId || Number.isNaN(proposalId)) {
    console.error('[post-dao-proposal] --proposal-id is required')
    printUsage()
    process.exit(1)
  }

  const canBroadcast = useServer
    ? true
    : process.env.JUNO_REPLY_BOT_MNEMONIC || process.env.MOULTBOOK_DRY_RUN === 'true'

  if (!canBroadcast) {
    console.error('[post-dao-proposal] either set JUNO_REPLY_BOT_MNEMONIC, MOULTBOOK_DRY_RUN=true, or use --use-server')
    printUsage()
    process.exit(1)
  }

  console.log(`[post-dao-proposal] fetching proposal A${proposalId} from ${PROPOSAL_MODULE}`)
  const body = await fetchProposal(proposalId)

  let overrideText = null
  if (file) {
    overrideText = readFileSync(file, 'utf8')
    console.log(`[post-dao-proposal] using local description override: ${file}`)
  }

  const envelope = buildAkbEnvelope({ id: proposalId, body, overrideText, tags, extraRefs: refs })

  let result
  if (useServer) {
    console.log(`[post-dao-proposal] ${preview ? 'previewing' : 'posting'} AKB export via ${REPLY_BOT_SERVER_URL}`)
    result = await postViaServer(envelope, preview)
  } else {
    console.log(`[post-dao-proposal] posting AKB export (mime ${envelope.content.mime_type})`)
    result = await postAkbExportToMoultbook(envelope)
  }
  console.log(JSON.stringify(result, null, 2))
}

main().catch((err) => {
  console.error('[post-dao-proposal] failed:', err.message)
  process.exit(1)
})
