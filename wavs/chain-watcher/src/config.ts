// ── Chain Watcher Configuration ──

export const CONFIG = {
  // Juno testnet
  chainId: 'uni-7',
  rpcHttp: process.env.JUNO_RPC || 'https://juno-testnet-rpc.polkachu.com',
  rpcWs: process.env.JUNO_WS || 'wss://juno-testnet-rpc.polkachu.com/websocket',
  restApi: process.env.JUNO_REST || 'https://juno-testnet-api.polkachu.com',
  denom: 'ujunox',
  gasPrice: '0.025ujunox',

  // Contracts to watch
  agentCompany: process.env.AGENT_COMPANY || 'juno1k8dxll425mcclacaxhrmkx9w5pznx9w5ggmw53tpj0c009ngfnjstj85k6',

  // Operator wallet (mnemonic or path to keyfile)
  operatorMnemonic: process.env.OPERATOR_MNEMONIC || '',

  // WebSocket feed port for frontend
  feedPort: Number(process.env.FEED_PORT) || 7778,

  // Polling fallback interval (ms) when WS is unavailable
  pollInterval: Number(process.env.POLL_INTERVAL) || 6000,

  // Events to watch for
  watchEvents: [
    'wasm-wavs_push',
    'wasm-outcome_create',
    'wasm-sortition_request',
    'wasm-code_upgrade',
    'wasm-execute_proposal',
    'wasm-create_proposal',
    'wasm-cast_vote',
  ],

  // Log level
  logLevel: (process.env.LOG_LEVEL || 'info') as 'debug' | 'info' | 'warn' | 'error',
} as const

export type Config = typeof CONFIG
