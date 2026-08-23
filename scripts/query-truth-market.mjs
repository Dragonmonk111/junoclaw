import { join, dirname } from 'path'
import { fileURLToPath, pathToFileURL } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const MCP_DIST = join(__dirname, '..', 'mcp', 'dist')

function cosmImport(pkg) {
  return import(pathToFileURL(join(MCP_DIST, '..', 'node_modules', '@cosmjs', pkg, 'build', 'index.js')).href)
}

const TRUTH_MARKET = 'juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p'
const RPC = 'https://juno.rpc.t.stavr.tech'

const { CosmWasmClient } = await cosmImport('cosmwasm-stargate')
const c = await CosmWasmClient.connect(RPC)

const stats = await c.queryContractSmart(TRUTH_MARKET, { get_stats: {} })
console.log('=== Truth Market Stats ===')
console.log(JSON.stringify(stats, (k, v) => typeof v === 'bigint' ? v.toString() : v, 2))

const ops = await c.queryContractSmart(TRUTH_MARKET, { list_operators: {} })
console.log('\n=== Operators ===')
for (const op of ops.operators) {
  console.log(`  ${op.address}`)
  console.log(`    fingerprint: ${op.fingerprint}`)
  console.log(`    stake: ${op.stake} ujunox`)
  console.log(`    active: ${op.active}`)
  console.log(`    verdicts: ${op.verdicts_submitted || 'n/a'}`)
  console.log('')
}

// Check for current epoch / pending batches
try {
  const epoch = await c.queryContractSmart(TRUTH_MARKET, { get_current_epoch: {} })
  console.log('=== Current Epoch ===')
  console.log(JSON.stringify(epoch, (k, v) => typeof v === 'bigint' ? v.toString() : v, 2))
} catch (e) {
  console.log('No current epoch query:', e.message)
}

// Check operator details for our DAO operator
try {
  const opDetail = await c.queryContractSmart(TRUTH_MARKET, { get_operator: { address: 'juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd' } })
  console.log('=== DAO Operator Detail ===')
  console.log(JSON.stringify(opDetail, (k, v) => typeof v === 'bigint' ? v.toString() : v, 2))
} catch (e) {
  console.log('No get_operator query:', e.message)
}
