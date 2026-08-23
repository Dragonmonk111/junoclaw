import { join, dirname } from 'path'
import { fileURLToPath, pathToFileURL } from 'url'

const __dirname = dirname(fileURLToPath(import.meta.url))
const MCP_DIST = join(__dirname, '..', 'mcp', 'dist')

function cosmImport(pkg) {
  return import(pathToFileURL(join(MCP_DIST, '..', 'node_modules', '@cosmjs', pkg, 'build', 'index.js')).href)
}

const TRUTH_MARKET = 'juno1rsf3uykfj6qqnzjhsaur8zgctrkapxhx0e7p507v2rh77v8kv37q8gqe8p'
const RPC = 'https://juno.rpc.t.stavr.tech'
const OPERATOR = 'juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd'

const { CosmWasmClient } = await cosmImport('cosmwasm-stargate')
const c = await CosmWasmClient.connect(RPC)

// Check epochs 1-10
for (let h = 1; h <= 10; h++) {
  try {
    const epoch = await c.queryContractSmart(TRUTH_MARKET, { get_epoch: { batch_height: h } })
    console.log(`Epoch ${h}:`, JSON.stringify(epoch, (k, v) => typeof v === 'bigint' ? v.toString() : v))
  } catch (e) {
    console.log(`Epoch ${h}: not found (${e.message.substring(0, 80)})`)
  }
}

// Check config for admin/relayer
try {
  const config = await c.queryContractSmart(TRUTH_MARKET, { get_config: {} })
  console.log('\nConfig:', JSON.stringify(config, (k, v) => typeof v === 'bigint' ? v.toString() : v, 2))
} catch (e) {
  console.log('Config query failed:', e.message)
}
