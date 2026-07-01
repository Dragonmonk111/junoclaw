import { writeFileSync, existsSync, mkdirSync } from 'fs'
import { dirname, join } from 'path'
import { fileURLToPath, pathToFileURL } from 'url'
import { renderDigest } from './render-rich.js'

const __filename = fileURLToPath(import.meta.url)
const __dirname = dirname(__filename)

// ── Configuration ───────────────────────────────────────────────────────────

export const DAO_CORE = process.env.DAO_CORE || 'juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac'
export const PROPOSAL_MODULE = process.env.PROPOSAL_MODULE || 'juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp'
export const REST_ENDPOINT = process.env.REST_ENDPOINT || 'https://juno-rest.publicnode.com'
const MOULT_ID = process.env.MOULT_ID || null
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

export async function getVotingModule() {
  try {
    const resp = await querySmart(DAO_CORE, { voting_module: {} })
    const data = extractData(resp)
    return typeof data === 'string' ? data : data.addr
  } catch (e) {
    console.warn('Could not query voting_module from DAO core:', e.message)
    return null
  }
}

async function getNftContract(votingModule) {
  if (!votingModule) return null
  try {
    // DAO DAO cw721-roles voting module stores the NFT contract in its config.
    const resp = await querySmart(votingModule, { config: {} })
    const cfg = extractData(resp)
    return cfg.nft_address || null
  } catch (e) {
    console.warn('Could not query voting module config:', e.message)
    return null
  }
}

async function getCw721Members(nftContract) {
  if (!nftContract) return []
  try {
    const tokens = extractData(await querySmart(nftContract, { all_tokens: {} })).tokens || []
    const members = []
    for (const tokenId of tokens) {
      try {
        const info = extractData(await querySmart(nftContract, { all_nft_info: { token_id: tokenId } }))
        const ext = info.info?.extension || {}
        members.push({
          addr: info.access?.owner || '',
          weight: Number(ext.weight || 0),
          role: ext.role || tokenId.split(':')[0] || 'member',
          token_id: tokenId,
        })
      } catch (e) {
        console.warn(`Could not query token ${tokenId}:`, e.message)
      }
    }
    return members
  } catch (e) {
    console.warn('Could not query cw721 members:', e.message)
    return []
  }
}

export async function getMembers(votingModule) {
  if (!votingModule) return []
  const nftContract = await getNftContract(votingModule)
  if (nftContract) return getCw721Members(nftContract)
  // Fallback for older token-weighted voting modules.
  try {
    const resp = await querySmart(votingModule, { list_members: { limit: 100 } })
    return extractData(resp).members || []
  } catch (e) {
    console.warn('Could not query members:', e.message)
    return []
  }
}

export async function getProposals() {
  try {
    const resp = await querySmart(PROPOSAL_MODULE, { list_proposals: { limit: 100 } })
    return extractData(resp).proposals || []
  } catch (e) {
    console.warn('Could not query proposals:', e.message)
    return []
  }
}

export async function getTreasury() {
  try {
    const resp = await queryBankBalances(DAO_CORE)
    return resp.balances || []
  } catch (e) {
    console.warn('Could not query treasury:', e.message)
    return []
  }
}

// ── Rendering ───────────────────────────────────────────────────────────────

function normalizeProposal(p) {
  // DAO DAO proposal modules wrap the proposal body under a `proposal` key.
  const body = p.proposal || p

  const rawVotes = body.votes || {}
  const votes = {
    yes: Number(rawVotes.yes || 0),
    no: Number(rawVotes.no || 0),
    abstain: Number(rawVotes.abstain || 0),
    total: 0,
    threshold: Number(body.threshold?.absolute_percentage?.percentage || 0),
  }
  votes.total = votes.yes + votes.no + votes.abstain

  const createdAt = body.created_at || body.start_time || body.start_height
  const newToday = isNewToday(createdAt)
  const closingSoon = body.status === 'open' && isClosingSoon(body.expiration)

  return {
    id: p.id,
    title: body.title || '(no title)',
    status: body.status,
    created_at: createdAt,
    expiration: body.expiration,
    votes,
    is_new_today: newToday,
    is_closing_soon: closingSoon,
  }
}

export function buildDigestData({ date, proposals, members, treasury, moultId }) {
  const enriched = proposals.map(normalizeProposal)
  const totalPower = members.reduce((sum, m) => sum + Number(m.weight || 0), 0)

  const summary = {
    total_proposals: enriched.length,
    open: enriched.filter((p) => p.status === 'open').length,
    passed: enriched.filter((p) => p.status === 'passed').length,
    ready_to_execute: enriched.filter((p) => p.status === 'passed').length,
    closed: enriched.filter((p) =>
      ['executed', 'rejected', 'closed', 'execution_failed'].includes(p.status),
    ).length,
    needs_votes: enriched.filter((p) => p.status === 'open').length,
    closing_soon: enriched.filter((p) => p.status === 'open' && p.is_closing_soon).length,
    new_today: enriched.filter((p) => p.is_new_today).length,
    total_voting_power: totalPower,
  }

  return {
    date,
    summary,
    proposals: enriched,
    members: members.map((m) => ({
      addr: m.addr,
      weight: Number(m.weight || 0),
      role: m.role || null,
    })),
    treasury: treasury.map((b) => ({
      denom: b.denom,
      amount: b.amount,
    })),
    meta: {
      dao_core: DAO_CORE,
      proposal_module: PROPOSAL_MODULE,
      moultbook: MOULT_ID || null,
      rest_endpoint: REST_ENDPOINT,
      generated_at: new Date().toISOString(),
    },
  }
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

  const data = buildDigestData({ date, proposals, members, treasury, moultId: MOULT_ID })
  const { plain, rich } = renderDigest(data)

  if (DRY_RUN) {
    console.log('\n--- DRY RUN ---')
    console.log(JSON.stringify(data, null, 2))
    console.log('\n--- RICH MARKDOWN ---')
    console.log(rich)
    console.log('\n--- END DRY RUN ---')
    return
  }

  if (!existsSync(DIGESTS_DIR)) {
    mkdirSync(DIGESTS_DIR, { recursive: true })
  }

  const latestPath = join(DIGESTS_DIR, 'latest.md')
  const datedPath = join(DIGESTS_DIR, `${date}.md`)
  const plainPath = join(DIGESTS_DIR, 'latest-plain.md')
  const jsonPath = join(DIGESTS_DIR, 'latest.json')

  writeFileSync(latestPath, rich, 'utf8')
  writeFileSync(datedPath, rich, 'utf8')
  writeFileSync(plainPath, plain, 'utf8')
  writeFileSync(jsonPath, JSON.stringify(data, null, 2), 'utf8')

  console.log(`Wrote ${latestPath}`)
  console.log(`Wrote ${datedPath}`)
  console.log(`Wrote ${plainPath}`)
  console.log(`Wrote ${jsonPath}`)
}

const isMainModule = process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href

if (isMainModule) {
  main().catch((err) => {
    console.error('Heartbeat digest failed:', err)
    process.exit(1)
  })
}
