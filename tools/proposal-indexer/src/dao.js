import { queryContract } from '../../context-agent/src/moultbook.js'
import { PROPOSAL_MODULE } from './config.js'

export async function fetchProposals(module = PROPOSAL_MODULE, limit = 100) {
  const resp = await queryContract(module, { list_proposals: { limit } })
  return resp.proposals || []
}

export async function fetchProposal(proposalId, module = PROPOSAL_MODULE) {
  const resp = await queryContract(module, { proposal: { proposal_id: proposalId } })
  return resp.proposal || resp
}
