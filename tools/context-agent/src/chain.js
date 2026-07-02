import { readFileSync } from 'fs'
import { join, dirname } from 'path'
import { fileURLToPath } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const INDEX_FILE = process.env.INDEX_FILE || join(__dirname, '..', 'cache', 'index.json')

export function loadIndex() {
  return JSON.parse(readFileSync(INDEX_FILE, 'utf8'))
}

export function buildChain(index, fromId, maxDepth = 100) {
  const chain = []
  let currentId = fromId || index.chain[0]
  let depth = 0

  while (currentId && depth < maxDepth) {
    const entry = index.by_id[currentId]
    if (!entry) break

    chain.push(entry)
    const refs = entry.refs || []
    currentId = refs[0] || null
    depth++
  }

  return chain
}

export function latestEntry(index) {
  return index.by_id[index.chain[0]]
}
