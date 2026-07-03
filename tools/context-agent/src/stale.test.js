import assert from 'node:assert/strict'
import test from 'node:test'
import { computeStaleMap, isStale, staleInfo, REDMARK_TYPE, UNREDMARK_TYPE } from './stale.js'

// Minimal fake index matching the shape indexer.js/trust.js expect.
function makeIndex(entries) {
  const by_id = {}
  const by_content_type = {}
  const by_author = {}
  const by_ref = {}
  for (const e of entries) {
    by_id[e.id] = e
    ;(by_content_type[e.content_type] ||= []).push(e.id)
    ;(by_author[e.author] ||= []).push(e.id)
    for (const r of e.refs || []) (by_ref[r] ||= []).push(e.id)
  }
  return { by_id, by_content_type, by_author, by_ref, by_voter: {}, by_disclosed_primary: {} }
}

// 5 votes -> trust score += 10, comfortably clears MIN_TRUST_SCORE (default 10)
// on its own, regardless of the small entries/reply_count contribution from
// the redmark post itself.
function makeTrusted(index, wallet) {
  index.by_voter[wallet] = [1, 2, 3, 4, 5]
}

test('untrusted wallet redmark is not honored', () => {
  const index = makeIndex([
    { id: 'moult:target', author: 'juno1author', content_type: 'application/json+agent-insight', refs: [], posted_at: '1000000000' },
    { id: 'moult:rm1', author: 'juno1untrusted', content_type: REDMARK_TYPE, refs: ['moult:target'], posted_at: '2000000000' },
  ])
  const staleMap = computeStaleMap(index)
  assert.equal(isStale(staleMap, 'moult:target'), false)
})

test('trusted wallet redmark is honored and annotated', () => {
  const index = makeIndex([
    { id: 'moult:target', author: 'juno1author', content_type: 'application/json+agent-insight', refs: [], posted_at: '1000000000' },
    { id: 'moult:rm1', author: 'juno1trusted', content_type: REDMARK_TYPE, refs: ['moult:target'], posted_at: '2000000000' },
  ])
  makeTrusted(index, 'juno1trusted')
  const staleMap = computeStaleMap(index)
  assert.equal(isStale(staleMap, 'moult:target'), true)
  const info = staleInfo(staleMap, 'moult:target')
  assert.equal(info.is_stale, true)
  assert.equal(info.marked_by, 'juno1trusted')
  assert.equal(info.redmark_id, 'moult:rm1')
})

test('later unredmark from another trusted wallet reverses it', () => {
  const index = makeIndex([
    { id: 'moult:target', author: 'juno1author', content_type: 'application/json+agent-insight', refs: [], posted_at: '1000000000' },
    { id: 'moult:rm1', author: 'juno1trusted-a', content_type: REDMARK_TYPE, refs: ['moult:target'], posted_at: '2000000000' },
    { id: 'moult:unrm1', author: 'juno1trusted-b', content_type: UNREDMARK_TYPE, refs: ['moult:target'], posted_at: '3000000000' },
  ])
  makeTrusted(index, 'juno1trusted-a')
  makeTrusted(index, 'juno1trusted-b')
  const staleMap = computeStaleMap(index)
  assert.equal(isStale(staleMap, 'moult:target'), false)
})

test('earlier unredmark does not override a later redmark', () => {
  const index = makeIndex([
    { id: 'moult:target', author: 'juno1author', content_type: 'application/json+agent-insight', refs: [], posted_at: '1000000000' },
    { id: 'moult:unrm1', author: 'juno1trusted-a', content_type: UNREDMARK_TYPE, refs: ['moult:target'], posted_at: '2000000000' },
    { id: 'moult:rm1', author: 'juno1trusted-b', content_type: REDMARK_TYPE, refs: ['moult:target'], posted_at: '3000000000' },
  ])
  makeTrusted(index, 'juno1trusted-a')
  makeTrusted(index, 'juno1trusted-b')
  const staleMap = computeStaleMap(index)
  assert.equal(isStale(staleMap, 'moult:target'), true)
})

test('an untrusted unredmark cannot undo an honored redmark', () => {
  const index = makeIndex([
    { id: 'moult:target', author: 'juno1author', content_type: 'application/json+agent-insight', refs: [], posted_at: '1000000000' },
    { id: 'moult:rm1', author: 'juno1trusted', content_type: REDMARK_TYPE, refs: ['moult:target'], posted_at: '2000000000' },
    { id: 'moult:unrm1', author: 'juno1untrusted', content_type: UNREDMARK_TYPE, refs: ['moult:target'], posted_at: '3000000000' },
  ])
  makeTrusted(index, 'juno1trusted')
  const staleMap = computeStaleMap(index)
  assert.equal(isStale(staleMap, 'moult:target'), true)
})

test('entry with no redmarks at all is not stale', () => {
  const index = makeIndex([
    { id: 'moult:target', author: 'juno1author', content_type: 'application/json+agent-insight', refs: [], posted_at: '1000000000' },
  ])
  const staleMap = computeStaleMap(index)
  assert.equal(isStale(staleMap, 'moult:target'), false)
  assert.deepEqual(staleInfo(staleMap, 'moult:target'), { is_stale: false, marked_by: null, at: null, redmark_id: null })
})
