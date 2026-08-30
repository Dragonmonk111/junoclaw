import { secp256k1 } from '@noble/curves/secp256k1'
import { sha256 } from '@noble/hashes/sha256'
import { bytesToHex, hexToBytes, utf8ToBytes } from '@noble/hashes/utils'

export type NostrTag = [string, string, ...unknown[]]

export interface UnsignedEvent {
  pubkey: string
  created_at: number
  kind: number
  tags: NostrTag[]
  content: string
}

export interface SignedEvent extends UnsignedEvent {
  id: string
  sig: string
}

function serializeEvent(ev: UnsignedEvent): string {
  return JSON.stringify([
    0,
    ev.pubkey,
    ev.created_at,
    ev.kind,
    ev.tags,
    ev.content,
  ])
}

export function getPubkey(privateKeyHex: string): string {
  const privKey = hexToBytes(privateKeyHex)
  return bytesToHex(secp256k1.getPublicKey(privKey, true).slice(1))
}

export function signEvent(ev: UnsignedEvent, privateKeyHex: string): SignedEvent {
  const serialized = serializeEvent(ev)
  const hash = sha256(utf8ToBytes(serialized))
  const privKey = hexToBytes(privateKeyHex)
  const sig = secp256k1.sign(hash, privKey, { lowS: false })
  const id = bytesToHex(hash)
  return {
    ...ev,
    id,
    sig: sig.toCompactHex(),
  }
}

export function createTextNote(
  privateKeyHex: string,
  content: string,
  tags: NostrTag[] = [],
): SignedEvent {
  const pubkey = getPubkey(privateKeyHex)
  return signEvent(
    {
      pubkey,
      created_at: Math.floor(Date.now() / 1000),
      kind: 1,
      tags,
      content,
    },
    privateKeyHex,
  )
}

export function createAuthEvent(
  privateKeyHex: string,
  challenge: string,
  relayUrl: string,
): SignedEvent {
  const pubkey = getPubkey(privateKeyHex)
  return signEvent(
    {
      pubkey,
      created_at: Math.floor(Date.now() / 1000),
      kind: 22242,
      tags: [
        ['relay', relayUrl],
        ['challenge', challenge],
      ],
      content: '',
    },
    privateKeyHex,
  )
}
