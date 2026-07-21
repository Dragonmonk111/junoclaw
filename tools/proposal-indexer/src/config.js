export const DAO_CORE =
  process.env.DAO_CORE || 'juno18k65at7fkf8elhece0fnhsvuxggqg6cved6trp5fyk3lftfn93xsmpeaac'

export const PROPOSAL_MODULE =
  process.env.PROPOSAL_MODULE || 'juno1jar50ltryvzp6axanam3v6gwsxakp2edmrz0n4r7y7h3hcwarp3sm6ccsp'

export const REST_ENDPOINT =
  process.env.REST_ENDPOINT || 'https://juno-rest.publicnode.com'

export const MOTHER_MOULT_ID =
  process.env.MOTHER_MOULT_ID || 'moult:49b0b8f5ee0832959920f6432bd6c15cc7551c07c48980a6efb0b28e657c7e2a'

export const REPLY_BOT_NAME =
  process.env.REPLY_BOT_NAME || 'dragonmonk111-bot'

export const DAO_DAO_BASE =
  process.env.DAO_DAO_BASE || 'https://dao.daodao.zone'

export const OVERRIDE_DIR =
  process.env.PROPOSAL_OVERRIDE_DIR || 'C:/cosmos-node/node-data/config/CascadeProjects/windsurf-project/junoclaw/drafts'

export const DEFAULT_TAGS =
  (process.env.PROPOSAL_DEFAULT_TAGS || 'dao-proposal,juno').split(',').map((t) => t.trim()).filter(Boolean)

export const DEFAULT_REFS =
  (process.env.PROPOSAL_DEFAULT_REFS || MOTHER_MOULT_ID).split(',').map((r) => r.trim()).filter(Boolean)

export const POLL_INTERVAL_MS = Number(process.env.POLL_INTERVAL_MS || '300000')

export const GAS_BUDGET_UJUNO = Number(process.env.GAS_BUDGET_UJUNO || '1000000')
