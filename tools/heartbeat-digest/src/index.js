import { writeFileSync, existsSync, mkdirSync } from 'fs'
import { dirname, join } from 'path'
import { fileURLToPath } from 'url'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

// ── Configuration ───────────────────────────────────────────────────────────

const DAO_CORE = process.env.DAO_CORE || 'juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac'
const PROPOSAL_MODULE = process.env.PROPOSAL_MODULE || 'juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp'
const REST_ENDPOINT = process.env.REST_ENDPOINT || 'https://juno-rest.publicnode.com'
const DRY_RUN = process.env.DRY_RUN === 'true'

const DIGESTS_DIR = join(__dirname, '..', 'digests')

// ── Helpers ─────────────────────────────────────────────────────────────────

function todayISO() {
  return new Date().toISOString().split('T')[0]
}

function formatDate(iso) {
  return new Date(iso).toISOString().replace('T', ' ').slice(0, 19) + ' UTC'
}

async function querySmart(contract, msg) {
  const query = Buffer.from(JSON.stringify(msg)).toString('base64')
  const url = `${REST_ENDPOINT}/cosmwasm/wasm/v1/contract/${contract}/smart/${query}`
  const res = await fetch(url)
  if (!res.ok) {
    const text = await res.text()
    throw new Error(`query failed ${res.status}: ${text} (url: ${url})`)
  }
  return res.json()
}

async function queryBankBalances(address) {
  const url = `${REST_ENDPOINT}/cosmos/bank/v1beta1/balances/${address}`
  const res = await fetch(url)
  if (!res.ok) {
    const text = await res.text()
    throw new Error(`balances query failed ${res.status}: ${text}`)
  }
  return res.json()
}

function extractData(response) {
  // CosmWasm REST responses wrap the actual data under .data
  return response.data ?? response
}

function statusLabel(status) {
  // Normalize DAO DAO proposal statuses
  const map = {
    open: 'Open',
    passed: 'Passed (ready to execute)',
    executed: 'Executed',
    rejected: 'Rejected',
    closed: 'Closed',
    execution_failed: 'Execution failed',
  }
  return map[status] || status
}

function isClosingSoon(expiration) {
  if (!expiration) return false
  const now = Date.now()
  const soon = now + 24 * 60 * 60 * 1000
  if (expiration.at_time) {
    const ms = parseInt(expiration.at_time) / 1_000_000
    return ms > now && ms < soon
  }
  return false
}

function isNewToday(createdAt) {
  if (!createdAt) return false
  const created = new Date(createdAt).toISOString().split('T')[0]
  return created === todayISO()
}

// ── Fetchers ────────────────────────────────────────────────────────────────

async function getVotingModule() {
  try {
    const resp = await querySmart(DAO_CORE, { voting_module: {} })
    return extractData(resp).addr
  } catch (e) {
    console.warn('Could not query voting_module from DAO core:', e.message)
    return null
  }
}

async function getMembers(votingModule) {
  if (!votingModule) return []
  try {
    const resp = await querySmart(votingModule, { list_members: { limit: 100 } })
    return extractData(resp).members || []
  } catch (e) {
    console.warn('Could not query members:', e.message)
    return []
  }
}

async function getProposals() {
  try {
    const resp = await querySmart(PROPOSAL_MODULE, { list_proposals: { limit: 100 } })
    return extractData(resp).proposals || []
  } catch (e) {
    console.warn('Could not query proposals:', e.message)
    return []
  }
}

async function getTreasury() {
  try {
    const resp = await queryBankBalances(DAO_CORE)
    return resp.balances || []
  } catch (e) {
    console.warn('Could not query treasury:', e.message)
    return []
  }
}

// ── Rendering ───────────────────────────────────────────────────────────────

function renderDigest({ date, proposals, members, treasury }) {
  const newToday = proposals.filter((p) => isNewToday(p.created_at || p.start_time))
  const needsVotes = proposals.filter((p) => p.status === 'open')
  const readyToExecute = proposals.filter((p) => p.status === 'passed')
  const closingSoon = proposals.filter((p) => p.status === 'open' && isClosingSoon(p.expiration))
  const closed = proposals.filter((p) =>
    ['executed', 'rejected', 'closed', 'execution_failed'].includes(p.status)
  )

  const treasuryLines = treasury.length
    ? treasury.map((b) => `- ${b.amount} ${b.denom}`).join('\n')
    : '- 0 ujuno (query failed)'

  const totalPower = members.reduce((sum, m) => sum + Number(m.weight || 0), 0)
  const memberLines = members.length
    ? members.map((m) => `- ${m.addr} — weight ${m.weight}`).join('\n')
    : '- Members query unavailable'

  return `# Juno Agents DAO Heartbeat Digest — ${date}

_Generated from on-chain data via public RPC._

## Quick stats
| Metric | Value |
|---|---|
| DAO core | ${DAO_CORE} |
| Total proposals | ${proposals.length} |
| Open | ${needsVotes.length} |
| Passed / ready to execute | ${readyToExecute.length} |
| Closed | ${closed.length} |
| Total voting power | ${totalPower} |

## New today
${newToday.length
    ? newToday.map((p) => `- **A${p.id}**: ${p.title || '(no title)'} — ${statusLabel(p.status)}`).join('\n')
    : '- none'}

## Needs votes
${needsVotes.length
    ? needsVotes.map((p) => `- **A${p.id}**: ${p.title || '(no title)'} [vote](https://dao.daodao.zone/dao/${DAO_CORE}/proposals/${p.id})`).join('\n')
    : '- none'}

## Ready to execute
${readyToExecute.length
    ? readyToExecute.map((p) => `- **A${p.id}**: ${p.title || '(no title)'}`).join('\n')
    : '- none'}

## Closing soon (next 24h)
${closingSoon.length
    ? closingSoon.map((p) => `- **A${p.id}**: ${p.title || '(no title)'}`).join('\n')
    : '- none'}

## Closed since last digest
${closed.length
    ? closed.map((p) => `- **A${p.id}**: ${p.title || '(no title)'} — ${statusLabel(p.status)}`).join('\n')
    : '- none'}

## Treasury
${treasuryLines}

## Members
${memberLines}

## Data sources
- DAO core: ${DAO_CORE}
- Proposal module: ${PROPOSAL_MODULE}
- REST endpoint: ${REST_ENDPOINT}
- Generated at: ${formatDate(new Date().toISOString())}
`
}

// ── Main ────────────────────────────────────────────────────────────────────

async function main() {
  console.log(`Starting heartbeat digest for ${DAO_CORE}`)
  console.log(`REST endpoint: ${REST_ENDPOINT}`)

  const date = todayISO()
  const votingModule = await getVotingModule()
  const [proposals, members, treasury] = await Promise.all([
    getProposals(),
    getMembers(votingModule),
    getTreasury(),
  ])

  const digest = renderDigest({ date, proposals, members, treasury })

  if (DRY_RUN) {
    console.log('\n--- DRY RUN ---')
    console.log(digest)
    console.log('--- END DRY RUN ---')
    return
  }

  if (!existsSync(DIGESTS_DIR)) {
    mkdirSync(DIGESTS_DIR, { recursive: true })
  }

  const latestPath = join(DIGESTS_DIR, 'latest.md')
  const datedPath = join(DIGESTS_DIR, `${date}.md`)

  writeFileSync(latestPath, digest, 'utf8')
  writeFileSync(datedPath, digest, 'utf8')

  console.log(`Wrote ${latestPath}`)
  console.log(`Wrote ${datedPath}`)
}

main().catch((err) => {
  console.error('Heartbeat digest failed:', err)
  process.exit(1)
})
