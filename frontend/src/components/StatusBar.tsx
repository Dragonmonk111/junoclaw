import { useStore } from '../store'
import { useChainClient } from '../hooks/useChainClient'
import { Wallet, LogOut, Loader2 } from 'lucide-react'

function truncAddr(addr: string) {
  return addr.length > 20 ? `${addr.slice(0, 10)}...${addr.slice(-6)}` : addr
}

function formatBalance(microAmount: string): string {
  const n = Number(microAmount) / 1_000_000
  return n < 0.01 ? '0' : n.toLocaleString(undefined, { maximumFractionDigits: 2 })
}

export function StatusBar() {
  const connected     = useStore((s) => s.connected)
  const daemonVersion = useStore((s) => s.daemonVersion)
  const agents        = useStore((s) => s.agents)

  const chain = useChainClient()

  return (
    <footer
      className="flex items-center justify-between px-4 py-1.5 text-[10px]"
      style={{
        background: '#08080e',
        borderTop: '1px solid rgba(255,255,255,0.04)',
        color: '#6b6a8a',
      }}
    >
      <div className="flex items-center gap-4">
        {/* Daemon connection status */}
        <div className="flex items-center gap-1.5">
          <span
            className="h-1.5 w-1.5 rounded-full"
            style={{
              background: connected ? '#00d4aa' : '#ef4444',
              boxShadow: connected
                ? '0 0 6px rgba(0,212,170,0.7)'
                : '0 0 6px rgba(239,68,68,0.7)',
            }}
          />
          <span style={{ color: connected ? '#00d4aa' : '#ef4444' }}>
            {connected ? 'Connected' : 'Disconnected'}
          </span>
        </div>

        {daemonVersion && (
          <span className="opacity-60">daemon v{daemonVersion}</span>
        )}

        <span className="opacity-60">
          {agents.length} agent{agents.length !== 1 ? 's' : ''}
        </span>

        {/* Chain query status */}
        {chain.lastFetched && (
          <span className="opacity-40">
            chain synced {Math.round((Date.now() - chain.lastFetched) / 1000)}s ago
          </span>
        )}
        {chain.chainError && (
          <span className="text-red-400 opacity-70 truncate max-w-[200px]" title={chain.chainError}>
            RPC: {chain.chainError.slice(0, 40)}
          </span>
        )}
      </div>

      <div className="flex items-center gap-3">
        {/* Chain badge */}
        <div className="flex items-center gap-1.5">
          <span
            className="h-1.5 w-1.5 rounded-full"
            style={{ background: '#ff6b4a', boxShadow: '0 0 5px rgba(255,107,74,0.6)' }}
          />
          <span className="opacity-70">Juno V29 · uni-7</span>
        </div>

        {/* Wallet connect / status */}
        {chain.walletAddress ? (
          <div className="flex items-center gap-2">
            <span className="text-[#c0bfd8] font-mono">{truncAddr(chain.walletAddress)}</span>
            <span className="text-[#fbbf24]">{formatBalance(chain.walletBalance)} JUNOX</span>
            <button
              onClick={chain.disconnectWalletFn}
              className="flex items-center gap-1 rounded px-1.5 py-0.5 transition hover:bg-white/5"
              style={{ color: '#6b6a8a' }}
              title="Disconnect wallet"
            >
              <LogOut className="h-2.5 w-2.5" />
            </button>
          </div>
        ) : (
          <button
            onClick={chain.connectWallet}
            className="flex items-center gap-1.5 rounded px-2 py-1 font-semibold uppercase tracking-wider transition hover:opacity-80"
            style={{
              background: 'rgba(255,107,74,0.1)',
              border: '1px solid rgba(255,107,74,0.25)',
              color: '#ff6b4a',
              fontSize: '9px',
            }}
          >
            {chain.loading ? (
              <Loader2 className="h-2.5 w-2.5 animate-spin" />
            ) : (
              <Wallet className="h-2.5 w-2.5" />
            )}
            Connect Keplr
          </button>
        )}
        {chain.walletError && (
          <span className="text-red-400 text-[9px] max-w-[150px] truncate" title={chain.walletError}>
            {chain.walletError.slice(0, 30)}
          </span>
        )}

        {/* TX pending indicator */}
        {chain.txPending && (
          <div className="flex items-center gap-1 text-[#fbbf24]">
            <Loader2 className="h-2.5 w-2.5 animate-spin" />
            <span>TX pending...</span>
          </div>
        )}
      </div>
    </footer>
  )
}
