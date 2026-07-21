import { postAkbExportToMoultbook } from '../../reply-bot/src/moultbook.js'

export async function postProposalEnvelope(envelope) {
  return postAkbExportToMoultbook(envelope)
}
