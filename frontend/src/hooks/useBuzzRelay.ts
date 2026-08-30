// ── React hook: Buzz relay connection with simulation fallback ──
//
// Manages a WebSocket connection to a DAO-owned Buzz relay. When no relay
// URL is configured (or connection fails), falls back to the local
// simulation from buzz-sim.ts so the panel is always useful for demos
// and development.
//
// When a real relay is available, this hook:
// 1. Opens a Nostr WebSocket subscription (REQ message with filter)
// 2. Parses incoming EVENT messages into BuzzMessage shapes
// 3. Tracks relay health (liveness, event count, latency)

import { useState, useEffect, useCallback, useRef } from 'react'
import {
  useBuzzSimulation,
  type BuzzState,
  type ChannelId,
  type BuzzMessage,
} from '../lib/buzz-sim'
import {
  createTextNote,
  createAuthEvent,
  getPubkey,
} from '../lib/nostr-sign'

export interface BuzzRelayState extends BuzzState {
  isLive: boolean
  relayUrl: string | null
  error: string | null
  pubkey: string | null
  hasPrivateKey: boolean
  setPrivateKey: (key: string) => void
  connect: (url: string) => void
  disconnect: () => void
  sendMessage: (channel: ChannelId, content: string) => boolean
}

const STORAGE_KEY = 'junoclaw_buzz_relay_url'
const PRIVKEY_STORAGE_KEY = 'junoclaw_buzz_privkey'

const VALID_CHANNELS: ChannelId[] = ['governance', 'truth-market', 'robotics', 'dev']

type NostrTag = [string, string, ...unknown[]]

function parseChannel(event: { tags?: NostrTag[] }): ChannelId {
  const channelTag = event.tags?.find((t) => t[0] === 't')
  const raw = channelTag?.[1] || 'dev'
  return (VALID_CHANNELS.includes(raw as ChannelId) ? raw : 'dev') as ChannelId
}

function parseMessageKind(kind: number): BuzzMessage['kind'] {
  if (kind === 38402) return 'task_discovery'
  if (kind === 1) {
    // Heuristic: content starting with "Draft verdict" or containing "SubmitVerdict" maps to verdict types
    return 'text'
  }
  return 'text'
}

interface NostrEvent {
  id?: string
  pubkey?: string
  content?: string
  created_at?: number
  kind?: number
  tags?: NostrTag[]
}

function eventToMessage(event: NostrEvent, channel: ChannelId): BuzzMessage {
  const id = event.id || `evt-${Date.now()}`
  const pubkey = event.pubkey || 'unknown'
  const content = event.content || ''
  const createdAt = event.created_at || Math.floor(Date.now() / 1000)
  const replyTag = event.tags?.find((t) => t[0] === 'e')

  let kind = parseMessageKind(event.kind as number)
  if (kind === 'text') {
    if (/draft\s*verdict/i.test(content)) kind = 'verdict_draft'
    else if (/SubmitVerdict|submit.*on-chain/i.test(content)) kind = 'verdict_submit'
  }

  return {
    id,
    channel,
    authorPubkey: pubkey,
    authorName: pubkey.slice(0, 8),
    content,
    timestamp: createdAt * 1000,
    attested: false,
    kind,
    replyTo: replyTag?.[1] as string | undefined,
  }
}

export function useBuzzRelay(): BuzzRelayState {
  const sim = useBuzzSimulation()
  const [isLive, setIsLive] = useState(false)
  const [relayUrl, setRelayUrl] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [liveMessages, setLiveMessages] = useState<Record<ChannelId, BuzzMessage[]>>({
    governance: [],
    'truth-market': [],
    robotics: [],
    dev: [],
  })
  const [liveEventCount, setLiveEventCount] = useState(0)
  const [liveLatency, setLiveLatency] = useState<number | null>(null)
  const [privateKey, setPrivateKeyState] = useState<string | null>(null)
  const [pubkey, setPubkey] = useState<string | null>(null)
  const wsRef = useRef<WebSocket | null>(null)
  const eventCountRef = useRef(0)
  const connectTimeRef = useRef<number>(0)
  const pendingAuthRef = useRef<string | null>(null)
  const authedRef = useRef(false)

  const setPrivateKey = useCallback((key: string) => {
    const clean = key.trim().toLowerCase()
    setPrivateKeyState(clean)
    localStorage.setItem(PRIVKEY_STORAGE_KEY, clean)
    try {
      setPubkey(getPubkey(clean))
    } catch {
      setPubkey(null)
    }
  }, [])

  // Load saved private key on mount
  useEffect(() => {
    const saved = localStorage.getItem(PRIVKEY_STORAGE_KEY)
    if (saved) {
      setPrivateKey(saved)
    }
  }, [setPrivateKey])

  const connect = useCallback((url: string) => {
    if (wsRef.current) {
      wsRef.current.close()
      wsRef.current = null
    }

    // Reset live state
    setLiveMessages({ governance: [], 'truth-market': [], robotics: [], dev: [] })
    setLiveEventCount(0)

    try {
      const ws = new WebSocket(url)
      wsRef.current = ws
      connectTimeRef.current = Date.now()

      ws.onopen = () => {
        setIsLive(true)
        setError(null)
        setRelayUrl(url)
        localStorage.setItem(STORAGE_KEY, url)
        eventCountRef.current = 0
        authedRef.current = false
        setLiveLatency(Date.now() - connectTimeRef.current)
      }

      ws.onerror = () => {
        setError(`Failed to connect to ${url}`)
        setIsLive(false)
      }

      ws.onclose = () => {
        setIsLive(false)
        wsRef.current = null
      }

      ws.onmessage = (ev) => {
        try {
          const data = JSON.parse(ev.data)
          const msgType = data[0]

          // Handle NIP-42 AUTH challenge
          if (msgType === 'AUTH') {
            const challenge = data[1] as string
            if (!challenge) return
            pendingAuthRef.current = challenge

            if (privateKey) {
              const authEvent = createAuthEvent(privateKey, challenge, url)
              ws.send(JSON.stringify(['AUTH', authEvent]))
              authedRef.current = true

              // Now send REQ after auth
              const req = JSON.stringify([
                'REQ',
                'junoclaw-buzz',
                { kinds: [1, 38402], limit: 100 },
              ])
              ws.send(req)
            }
            return
          }

          // Handle OK response (for published events)
          if (msgType === 'OK') {
            return
          }

          // Handle EOSE (end of stored events)
          if (msgType === 'EOSE') {
            return
          }

          if (msgType !== 'EVENT') return

          const event = data[2] as NostrEvent
          if (!event) return

          eventCountRef.current++
          setLiveEventCount(eventCountRef.current)

          const channel = parseChannel(event)
          const msg = eventToMessage(event, channel)

          setLiveMessages((prev) => ({
            ...prev,
            [channel]: [...(prev[channel] || []), msg].slice(-100),
          }))
        } catch {
          // Ignore malformed messages
        }
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err))
      setIsLive(false)
    }
  }, [])

  const disconnect = useCallback(() => {
    if (wsRef.current) {
      wsRef.current.close()
      wsRef.current = null
    }
    setIsLive(false)
    setRelayUrl(null)
    setLiveMessages({ governance: [], 'truth-market': [], robotics: [], dev: [] })
    setLiveEventCount(0)
    authedRef.current = false
    localStorage.removeItem(STORAGE_KEY)
  }, [])

  const sendMessage = useCallback((channel: ChannelId, content: string): boolean => {
    if (!wsRef.current || wsRef.current.readyState !== WebSocket.OPEN) return false
    if (!privateKey) return false

    try {
      const event = createTextNote(privateKey, content, [
        ['t', channel],
      ])
      wsRef.current.send(JSON.stringify(['EVENT', event]))

      // Optimistically add to local messages
      const msg: BuzzMessage = {
        id: event.id,
        channel,
        authorPubkey: event.pubkey,
        authorName: pubkey?.slice(0, 8) || event.pubkey.slice(0, 8),
        content,
        timestamp: event.created_at * 1000,
        attested: false,
        kind: 'text',
      }
      setLiveMessages((prev) => ({
        ...prev,
        [channel]: [...(prev[channel] || []), msg].slice(-100),
      }))
      return true
    } catch {
      return false
    }
  }, [privateKey, pubkey])

  // Auto-connect if a URL was previously saved
  useEffect(() => {
    const saved = localStorage.getItem(STORAGE_KEY)
    const url = saved || 'wss://buzz.junoclaw.xyz/ws'
    if (!wsRef.current) {
      connect(url)
    }
    return () => {
      if (wsRef.current) {
        wsRef.current.close()
        wsRef.current = null
      }
    }
  }, [connect])

  // When live, use live data; otherwise fall back to simulation
  const baseState = isLive
    ? {
        ...sim,
        messages: liveMessages,
        relay: {
          ...sim.relay,
          connected: true,
          url: relayUrl || '',
          eventCount: liveEventCount,
          latencyMs: liveLatency,
          ownerPubkey: '36944fabbccca892a33778e133eac3e9def36ec520513e8e637cf5113706edfe',
        },
      }
    : sim

  return {
    ...baseState,
    isLive,
    relayUrl,
    error,
    pubkey,
    hasPrivateKey: !!privateKey,
    setPrivateKey,
    connect,
    disconnect,
    sendMessage,
  }
}
