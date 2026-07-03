// DAO governance reads for the trust layer (A18c-3 §2: "Participation history:
// number of proposals voted on ...").
//
// The DAO proposal module (dao-proposal-single) is the source of truth for who
// voted on what. It has no "votes by voter" index, so we do the standard thing:
// enumerate proposals, page each proposal's `list_votes`, and fold the result
// into a per-voter map off-chain. Bounded work (proposals grow slowly), and the
// result is fully re-derivable by anyone against the same contract.

import { queryContract } from './moultbook.js'

export const DAO_CORE =
  process.env.DAO_CORE || 'juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac'
export const PROPOSAL_MODULE =
  process.env.PROPOSAL_MODULE || 'juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp'

const PAGE_LIMIT = 30

export async function listProposals() {
  const resp = await queryContract(PROPOSAL_MODULE, { list_proposals: { limit: 100 } })
  return resp.proposals || []
}

async function listVotesForProposal(proposalId) {
  const votes = []
  let startAfter = null
  while (true) {
    const resp = await queryContract(PROPOSAL_MODULE, {
      list_votes: { proposal_id: proposalId, start_after: startAfter, limit: PAGE_LIMIT },
    })
    const page = resp.votes || []
    if (page.length === 0) break
    votes.push(...page)
    if (page.length < PAGE_LIMIT) break
    startAfter = page[page.length - 1].voter
  }
  return votes
}

/**
 * Build { by_voter: { wallet: [{ proposal_id, vote, power }] }, proposal_count }.
 * Best-effort: any single failed query is logged and skipped rather than
 * breaking the caller — trust is advisory, DAO reads must never break indexing.
 */
export async function buildVoteIndex() {
  const by_voter = {}
  let proposals = []
  try {
    proposals = await listProposals()
  } catch (e) {
    console.warn('[dao] list_proposals failed:', e.message)
    return { by_voter, proposal_count: 0 }
  }

  for (const p of proposals) {
    const id = p.id
    if (id === undefined || id === null) continue
    let votes = []
    try {
      votes = await listVotesForProposal(id)
    } catch (e) {
      console.warn(`[dao] list_votes failed for proposal ${id}:`, e.message)
      continue
    }
    for (const v of votes) {
      const wallet = v.voter
      if (!wallet) continue
      ;(by_voter[wallet] ||= []).push({
        proposal_id: id,
        vote: typeof v.vote === 'string' ? v.vote : v.vote?.vote || null,
        power: v.power ?? null,
      })
    }
  }

  return { by_voter, proposal_count: proposals.length }
}
