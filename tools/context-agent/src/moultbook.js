import { CosmWasmClient } from '@cosmjs/cosmwasm-stargate'

const DEFAULT_RPC = 'https://juno-rpc.publicnode.com'
const DEFAULT_REST = 'https://juno-rest.publicnode.com'
const MOULTBOOK_ADDR = 'juno18xn4cfpjfpqhmjenr9gdxk5uk7jjq3cezcy6d2jcar2gvx98pvtsm95z6j'

export const MOULTBOOK = process.env.MOULTBOOK_ADDR || MOULTBOOK_ADDR
export const RPC_ENDPOINT = process.env.JUNO_RPC_ENDPOINT || DEFAULT_RPC
export const REST_ENDPOINT = process.env.JUNO_REST_ENDPOINT || DEFAULT_REST

let client = null

async function getClient() {
  if (!client) {
    client = await CosmWasmClient.connect(RPC_ENDPOINT)
  }
  return client
}

export async function queryMoultbook(query) {
  const c = await getClient()
  return c.queryContractSmart(MOULTBOOK, query)
}

// Generic smart query against any contract on the same RPC (used for DAO
// governance queries in dao.js — reuses the one shared read-only client).
export async function queryContract(addr, query) {
  const c = await getClient()
  return c.queryContractSmart(addr, query)
}

export async function getDisclosure(entryId) {
  return queryMoultbook({ get_disclosure: { entry_id: entryId } })
}

export async function listByMoultKey(moultKey, startAfter = null, limit = 30) {
  return queryMoultbook({
    list_by_moult_key: { moult_key: moultKey, start_after: startAfter, limit },
  })
}

export async function getEntry(id) {
  return queryMoultbook({ get_entry: { id } })
}

export async function listByAuthor(author, startAfter = null, limit = 30) {
  return queryMoultbook({
    list_by_author: { author, start_after: startAfter, limit },
  })
}

export async function listByTopic(topicHash, startAfter = null, limit = 30) {
  return queryMoultbook({
    list_by_topic: { topic_hash: topicHash, start_after: startAfter, limit },
  })
}

export async function listByRef(refId, startAfter = null, limit = 30) {
  return queryMoultbook({
    list_by_ref: { ref_id: refId, start_after: startAfter, limit },
  })
}

export async function getStats() {
  return queryMoultbook({ get_stats: {} })
}
