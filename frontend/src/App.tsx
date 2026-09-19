import { useEffect, useState, useCallback, useRef } from 'react'
import { Sidebar } from './components/Sidebar'
import { StatusBar } from './components/StatusBar'
import { HomeScreen } from './components/HomeScreen'
import { DiscoverScreen } from './components/DiscoverScreen'
import { DetailScreen } from './components/DetailScreen'
import { SettingsModal } from './components/SettingsModal'
import { ScreenSkeleton } from './components/Skeleton'
import { useStore } from './store'
import { Home, Compass, Search, Bot, Building2, ArrowRight } from 'lucide-react'

type Screen =
  | { name: 'home' }
  | { name: 'discover' }
  | { name: 'detail'; kind: 'agent' | 'dao' | 'tool'; id: string }

export default function App() {
  const connect = useStore((s) => s.connect)
  const [screen, setScreen] = useState<Screen>({ name: 'home' })
  const [loading, setLoading] = useState(false)
  const [showSettings, setShowSettings] = useState(false)
  const [showSearch, setShowSearch] = useState(false)

  useEffect(() => {
    connect()
  }, [connect])

  const switchScreen = useCallback((next: Screen) => {
    setLoading(true)
    setScreen(next)
    setTimeout(() => setLoading(false), 250)
  }, [])

  // Keyboard navigation: 1=Home, 2=Discover, Esc=back, / = search
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return
      if (e.key === '1') switchScreen({ name: 'home' })
      else if (e.key === '2') switchScreen({ name: 'discover' })
      else if (e.key === 'Escape' && screen.name === 'detail') switchScreen({ name: 'home' })
      else if (e.key === '/') { e.preventDefault(); setShowSearch(true) }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [screen.name, switchScreen])

  const navigateHome = () => switchScreen({ name: 'home' })
  const navigateDiscover = () => switchScreen({ name: 'discover' })
  const navigateAgent = (id: string) => switchScreen({ name: 'detail', kind: 'agent', id })
  const navigateDao = (id: string) => switchScreen({ name: 'detail', kind: 'dao', id })
  const navigateTool = (id: string) => switchScreen({ name: 'detail', kind: 'tool', id })

  return (
    <div className="flex h-full flex-col bg-[#06060f]">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar onSelectAgent={navigateAgent} onOpenSettings={() => setShowSettings(true)} />
        <main className="flex flex-1 flex-col overflow-hidden">
          {/* Top nav bar */}
          <div className="flex items-center gap-1 px-4 pt-2 pb-0"
               style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
            <NavButton active={screen.name === 'home'} onClick={navigateHome}
                       icon={<Home className="h-3.5 w-3.5" />} label="Home" hotkey="1" />
            <NavButton active={screen.name === 'discover'} onClick={navigateDiscover}
                       icon={<Compass className="h-3.5 w-3.5" />} label="Discover" hotkey="2" />
            {screen.name === 'detail' && (
              <div className="flex items-center gap-1.5 rounded-t-lg px-4 py-2 text-xs font-medium"
                   style={{ color: '#f0eff8', borderBottom: '2px solid #ff6b4a', marginBottom: '-1px' }}>
                <span style={{ color: '#ff6b4a' }}>●</span>
                Detail
              </div>
            )}
            <div className="ml-auto flex items-center gap-2">
              <button onClick={() => setShowSearch(true)}
                      className="flex items-center gap-1.5 rounded-lg px-2.5 py-1.5 text-xs text-[#6b6a8a] transition hover:bg-white/5 hover:text-[#c0bfd8]">
                <Search className="h-3.5 w-3.5" />
                <span>Search</span>
                <kbd className="rounded px-1 py-0.5 text-[9px] font-mono"
                     style={{ background: 'rgba(255,255,255,0.04)', color: '#4a4a6a' }}>/
                </kbd>
              </button>
            </div>
          </div>

          {/* Screen content */}
          <div className="flex flex-1 overflow-hidden">
            {loading ? <ScreenSkeleton /> : (
              <>
                {screen.name === 'home' && (
                  <HomeScreen
                    onNavigate={navigateDiscover}
                    onSelectAgent={navigateAgent}
                    onSelectDao={navigateDao}
                    onOpenTool={navigateTool}
                  />
                )}
                {screen.name === 'discover' && (
                  <DiscoverScreen
                    onSelectAgent={navigateAgent}
                    onSelectDao={navigateDao}
                    onNavigateHome={navigateHome}
                  />
                )}
                {screen.name === 'detail' && (
                  <DetailScreen
                    kind={screen.kind}
                    id={screen.id}
                    onBack={navigateHome}
                  />
                )}
              </>
            )}
          </div>
        </main>
      </div>
      <StatusBar />
      {showSettings && <SettingsModal onClose={() => setShowSettings(false)} />}
      {showSearch && (
        <GlobalSearchModal
          onSelectAgent={(id) => { setShowSearch(false); navigateAgent(id) }}
          onSelectDao={(id) => { setShowSearch(false); navigateDao(id) }}
          onOpenTool={(id) => { setShowSearch(false); navigateTool(id) }}
          onClose={() => setShowSearch(false)}
        />
      )}
    </div>
  )
}

function GlobalSearchModal({ onSelectAgent, onSelectDao, onOpenTool, onClose }: {
  onSelectAgent: (id: string) => void
  onSelectDao: (id: string) => void
  onOpenTool: (id: string) => void
  onClose: () => void
}) {
  const agents = useStore((s) => s.agents)
  const daos = useStore((s) => s.daos)
  const [query, setQuery] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  const q = query.toLowerCase()
  const agentResults = q ? agents.filter((a) => a.name.toLowerCase().includes(q) || a.description.toLowerCase().includes(q)) : []
  const daoResults = q ? daos.filter((d) => d.name.toLowerCase().includes(q)) : []
  const portalResults = q ? [
    { id: 'chat', label: 'Agent Chat' }, { id: 'dao', label: 'DAO Governance' },
    { id: 'dex', label: 'DEX' }, { id: 'intel', label: 'Qu-Zeno Intel' },
    { id: 'robotops', label: 'Robot Ops' }, { id: 'feepay', label: 'FeePay' },
    { id: 'miner', label: 'Truth Miners' }, { id: 'buzz', label: 'Buzz Relay' },
    { id: 'commonwealth', label: 'Commonwealth' }, { id: 'contracts', label: 'Contracts' },
    { id: 'updates', label: 'Chain Updates' },
  ].filter((p) => p.label.toLowerCase().includes(q)) : []

  const hasResults = agentResults.length > 0 || daoResults.length > 0 || portalResults.length > 0

  return (
    <div className="fixed inset-0 z-50 flex items-start justify-center pt-32"
         style={{ background: 'rgba(6,6,15,0.8)', backdropFilter: 'blur(4px)' }}
         onClick={onClose}>
      <div className="w-full max-w-xl rounded-2xl p-4"
           style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.08)', boxShadow: '0 24px 64px rgba(0,0,0,0.5)' }}
           onClick={(e) => e.stopPropagation()}>
        <div className="relative">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-4 w-4 text-[#3a3a5a]" />
          <input ref={inputRef}
                 value={query}
                 onChange={(e) => setQuery(e.target.value)}
                 placeholder="Search agents, DAOs, portals…"
                 className="w-full rounded-xl py-3 pl-10 pr-4 text-sm text-white placeholder-[#3a3a5a] outline-none"
                 style={{ background: '#16162b', border: '1px solid rgba(255,255,255,0.06)' }}
                 onKeyDown={(e) => e.key === 'Escape' && onClose()} />
        </div>
        {query && (
          <div className="mt-3 max-h-80 overflow-y-auto">
            {agentResults.length > 0 && (
              <div className="mb-2">
                <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a]">Agents</div>
                {agentResults.slice(0, 5).map((a) => (
                  <button key={a.id} onClick={() => onSelectAgent(a.id)}
                          className="w-full flex items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm text-[#c0bfd8] transition hover:bg-white/5">
                    <Bot className="h-4 w-4 text-[#ff6b4a]" /> {a.name}
                  </button>
                ))}
              </div>
            )}
            {daoResults.length > 0 && (
              <div className="mb-2">
                <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a]">DAOs</div>
                {daoResults.slice(0, 5).map((d) => (
                  <button key={d.id} onClick={() => onSelectDao(d.id)}
                          className="w-full flex items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm text-[#c0bfd8] transition hover:bg-white/5">
                    <Building2 className="h-4 w-4 text-[#00d4aa]" /> {d.name}
                  </button>
                ))}
              </div>
            )}
            {portalResults.length > 0 && (
              <div className="mb-2">
                <div className="px-2 py-1 text-[10px] font-semibold uppercase tracking-wider text-[#6b6a8a]">Portals</div>
                {portalResults.map((p) => (
                  <button key={p.id} onClick={() => onOpenTool(p.id)}
                          className="w-full flex items-center gap-2.5 rounded-lg px-3 py-2 text-left text-sm text-[#c0bfd8] transition hover:bg-white/5">
                    <ArrowRight className="h-4 w-4 text-[#a78bfa]" /> {p.label}
                  </button>
                ))}
              </div>
            )}
            {!hasResults && (
              <div className="py-8 text-center text-sm text-[#6b6a8a]">No results for "{query}"</div>
            )}
          </div>
        )}
      </div>
    </div>
  )
}

function NavButton({ active, onClick, icon, label, hotkey }: {
  active: boolean
  onClick: () => void
  icon: React.ReactNode
  label: string
  hotkey?: string
}) {
  return (
    <button onClick={onClick}
            className="flex items-center gap-1.5 rounded-t-lg px-4 py-2 text-xs font-medium transition-all"
            style={active ? {
              color: '#f0eff8',
              background: 'rgba(255,107,74,0.07)',
              borderBottom: '2px solid #ff6b4a',
              marginBottom: '-1px',
            } : {
              color: '#6b6a8a',
              borderBottom: '2px solid transparent',
              marginBottom: '-1px',
            }}>
      <span style={{ color: active ? '#ff6b4a' : '#6b6a8a' }}>{icon}</span>
      {label}
      {hotkey && (
        <kbd className="ml-1 rounded px-1 py-0.5 text-[9px] font-mono"
             style={{ background: 'rgba(255,255,255,0.04)', color: '#4a4a6a' }}>
          {hotkey}
        </kbd>
      )}
    </button>
  )
}
