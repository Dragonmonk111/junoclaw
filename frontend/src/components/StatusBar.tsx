import { useStore } from '../store'

export function StatusBar() {
  const connected     = useStore((s) => s.connected)
  const daemonVersion = useStore((s) => s.daemonVersion)
  const agents        = useStore((s) => s.agents)

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
        {/* Connection status */}
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
      </div>

      <div className="flex items-center gap-4">
        {/* Chain badge */}
        <div className="flex items-center gap-1.5">
          <span
            className="h-1.5 w-1.5 rounded-full"
            style={{ background: '#ff6b4a', boxShadow: '0 0 5px rgba(255,107,74,0.6)' }}
          />
          <span className="opacity-70">Juno V29 · uni-7 testnet</span>
        </div>

        <span
          className="rounded px-1.5 py-0.5 font-semibold uppercase tracking-wider"
          style={{ background: 'rgba(0,212,170,0.08)', color: '#00d4aa', fontSize: '9px' }}
        >
          Local
        </span>
      </div>
    </footer>
  )
}
