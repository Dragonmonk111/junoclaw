import { useEffect, useState } from 'react'
import { Sidebar } from './components/Sidebar'
import { ChatPanel } from './components/ChatPanel'
import { DaoPanel } from './components/DaoPanel'
import { UpdatesPanel } from './components/UpdatesPanel'
import { StatusBar } from './components/StatusBar'
import { useStore } from './store'
import { MessageSquare, Building2, RefreshCw } from 'lucide-react'

type MainTab = 'chat' | 'dao' | 'updates'

const TABS: { id: MainTab; label: string; icon: React.ReactNode }[] = [
  { id: 'chat',    label: 'Chat',    icon: <MessageSquare className="h-3.5 w-3.5" /> },
  { id: 'dao',     label: 'DAO',     icon: <Building2     className="h-3.5 w-3.5" /> },
  { id: 'updates', label: 'Updates', icon: <RefreshCw     className="h-3.5 w-3.5" /> },
]

export default function App() {
  const connect = useStore((s) => s.connect)
  const [activeTab, setActiveTab] = useState<MainTab>('chat')

  useEffect(() => {
    connect()
  }, [connect])

  return (
    <div className="flex h-full flex-col bg-[#06060f]">
      <div className="flex flex-1 overflow-hidden">
        <Sidebar />
        <main className="flex flex-1 flex-col overflow-hidden">
          {/* Tab bar */}
          <div className="flex items-center gap-1 px-4 pt-2 pb-0"
               style={{ borderBottom: '1px solid rgba(255,255,255,0.05)' }}>
            {TABS.map((tab) => {
              const isActive = activeTab === tab.id
              return (
                <button
                  key={tab.id}
                  onClick={() => setActiveTab(tab.id)}
                  className="flex items-center gap-1.5 rounded-t-lg px-4 py-2 text-xs font-medium transition-all"
                  style={isActive ? {
                    color: '#f0eff8',
                    background: 'rgba(255,107,74,0.07)',
                    borderBottom: '2px solid #ff6b4a',
                    marginBottom: '-1px',
                  } : {
                    color: '#6b6a8a',
                    borderBottom: '2px solid transparent',
                    marginBottom: '-1px',
                  }}
                >
                  <span style={{ color: isActive ? '#ff6b4a' : '#6b6a8a' }}>{tab.icon}</span>
                  {tab.label}
                </button>
              )
            })}
          </div>

          {/* Panel */}
          <div className="flex flex-1 overflow-hidden">
            {activeTab === 'chat'    && <ChatPanel />}
            {activeTab === 'dao'     && <DaoPanel />}
            {activeTab === 'updates' && <UpdatesPanel />}
          </div>
        </main>
      </div>
      <StatusBar />
    </div>
  )
}
