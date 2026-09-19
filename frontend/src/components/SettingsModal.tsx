import { useState, useEffect, useRef } from 'react'
import { X, Globe, Cpu, Shield, ExternalLink } from 'lucide-react'
import { CHAIN_CONFIG } from '../lib/chain-config'

const STORAGE_KEY = 'junoclaw_settings'

interface Settings {
  rpc: string
  rest: string
  defaultModel: string
}

function loadSettings(): Settings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (raw) return JSON.parse(raw)
  } catch {}
  return {
    rpc: CHAIN_CONFIG.rpc,
    rest: CHAIN_CONFIG.rest,
    defaultModel: 'llama3.1:8b',
  }
}

function saveSettings(s: Settings) {
  try { localStorage.setItem(STORAGE_KEY, JSON.stringify(s)) } catch {}
}

const MODELS = [
  { value: 'llama3.2:3b',     label: 'Llama 3.2 3B',     tag: 'local' },
  { value: 'qwen2.5:1.5b',    label: 'Qwen 2.5 1.5B',    tag: 'local' },
  { value: 'llama3.1:8b',     label: 'Llama 3.1 8B',     tag: 'local' },
  { value: 'deepseek-r1:14b', label: 'DeepSeek R1 14B',  tag: 'local' },
  { value: 'claude-sonnet',   label: 'Claude Sonnet',     tag: 'cloud' },
  { value: 'gpt-4o',          label: 'GPT-4o',            tag: 'cloud' },
]

export function SettingsModal({ onClose }: { onClose: () => void }) {
  const [settings, setSettings] = useState<Settings>(loadSettings)
  const [saved, setSaved] = useState(false)
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    inputRef.current?.focus()
  }, [])

  const handleSave = () => {
    saveSettings(settings)
    setSaved(true)
    setTimeout(() => setSaved(false), 1500)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center"
         style={{ background: 'rgba(6,6,15,0.8)', backdropFilter: 'blur(4px)' }}
         onClick={onClose}>
      <div className="w-full max-w-md rounded-2xl p-6"
           style={{ background: '#0a0a18', border: '1px solid rgba(255,255,255,0.08)', boxShadow: '0 24px 64px rgba(0,0,0,0.5)' }}
           onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="flex items-center justify-between mb-5">
          <h2 className="text-base font-semibold text-white">Settings</h2>
          <button onClick={onClose} className="rounded-lg p-1.5 transition hover:bg-white/5">
            <X className="h-4 w-4 text-[#6b6a8a]" />
          </button>
        </div>

        {/* RPC Endpoint */}
        <div className="mb-4">
          <label className="flex items-center gap-1.5 mb-1.5 text-xs font-medium text-[#6b6a8a]">
            <Globe className="h-3.5 w-3.5" /> RPC Endpoint
          </label>
          <input ref={inputRef}
                 value={settings.rpc}
                 onChange={(e) => setSettings({ ...settings, rpc: e.target.value })}
                 className="w-full rounded-xl px-3 py-2.5 text-sm text-white outline-none"
                 style={{ background: '#16162b', border: '1px solid rgba(255,255,255,0.06)' }}
                 placeholder="https://..." />
        </div>

        {/* REST Endpoint */}
        <div className="mb-4">
          <label className="flex items-center gap-1.5 mb-1.5 text-xs font-medium text-[#6b6a8a]">
            <Globe className="h-3.5 w-3.5" /> REST Endpoint
          </label>
          <input value={settings.rest}
                 onChange={(e) => setSettings({ ...settings, rest: e.target.value })}
                 className="w-full rounded-xl px-3 py-2.5 text-sm text-white outline-none"
                 style={{ background: '#16162b', border: '1px solid rgba(255,255,255,0.06)' }}
                 placeholder="https://..." />
        </div>

        {/* Default Model */}
        <div className="mb-5">
          <label className="flex items-center gap-1.5 mb-1.5 text-xs font-medium text-[#6b6a8a]">
            <Cpu className="h-3.5 w-3.5" /> Default Model
          </label>
          <select value={settings.defaultModel}
                  onChange={(e) => setSettings({ ...settings, defaultModel: e.target.value })}
                  className="w-full rounded-xl px-3 py-2.5 text-sm text-white outline-none"
                  style={{ background: '#16162b', border: '1px solid rgba(255,255,255,0.06)' }}>
            {MODELS.map((m) => (
              <option key={m.value} value={m.value}>{m.label} ({m.tag})</option>
            ))}
          </select>
        </div>

        {/* Info box */}
        <div className="mb-5 rounded-xl p-3"
             style={{ background: 'rgba(96,165,250,0.06)', border: '1px solid rgba(96,165,250,0.12)' }}>
          <div className="flex items-start gap-2">
            <Shield className="h-4 w-4 text-blue-400 flex-shrink-0 mt-0.5" />
            <p className="text-xs text-[#6b6a8a] leading-relaxed">
              Settings are stored locally in your browser. RPC changes apply on next reconnect.
            </p>
          </div>
        </div>

        {/* Actions */}
        <div className="flex items-center gap-3">
          <button onClick={handleSave}
                  className="flex-1 rounded-xl py-2.5 text-sm font-semibold text-white transition hover:opacity-90"
                  style={{ background: 'linear-gradient(135deg, #ff6b4a, #e84e2c)' }}>
            {saved ? 'Saved!' : 'Save Settings'}
          </button>
          <button onClick={() => window.open('https://github.com/dragonmonk/junoclaw', '_blank')}
                  className="flex items-center gap-1.5 rounded-xl px-3 py-2.5 text-sm text-[#6b6a8a] transition hover:bg-white/5">
            <ExternalLink className="h-3.5 w-3.5" /> GitHub
          </button>
        </div>
      </div>
    </div>
  )
}
