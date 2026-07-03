import { mintKnowledgeMoult } from './knowledge-moults.js'

const AGENT_NAME = process.env.REPLY_BOT_NAME || 'dragonmonk111-bot'
const MOTIVE = process.argv[2] || process.env.MINT_MOTIVE
const KNOWLEDGE_SUMMARY = process.env.MINT_SUMMARY
const SOURCE_MOULTS = (process.env.MINT_SOURCE_MOULTS || '')
  .split(',')
  .map((s) => s.trim())
  .filter(Boolean)
const OWNER = process.env.MINT_OWNER || null

async function main() {
  if (!MOTIVE || !KNOWLEDGE_SUMMARY) {
    console.error('Usage: npm run mint -- "<motive>"   (with MINT_SUMMARY env var set)')
    console.error('   or: MINT_MOTIVE="..." MINT_SUMMARY="..." npm run mint')
    console.error('Optional: MINT_SOURCE_MOULTS="moult:a,moult:b" MINT_OWNER=juno1...')
    process.exit(1)
  }

  try {
    const result = await mintKnowledgeMoult({
      agent: AGENT_NAME,
      motive: MOTIVE,
      knowledgeSummary: KNOWLEDGE_SUMMARY,
      sourceMoults: SOURCE_MOULTS,
      owner: OWNER,
    })
    console.log(JSON.stringify(result, null, 2))
  } catch (e) {
    console.error('[reply-bot] mint failed:', e.message)
    process.exit(1)
  }
}

main()
