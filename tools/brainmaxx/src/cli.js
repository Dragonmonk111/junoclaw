#!/usr/bin/env node
// Brainmaxx v0 CLI dispatch (spec §12). D0-only: deterministic, no model
// embedded, no daemon, no autonomous posting.

import { pathToFileURL, fileURLToPath } from 'node:url'
import { readFileSync, writeFileSync, existsSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { createHash } from 'node:crypto'

import { getConfig, configHash, MIN_SCORE, DEFAULT_K } from './config.js'
import { loadSnapshot } from './store.js'
import { buildPack } from './pack.js'
import { createTrace, writeTrace, readTrace, resolveRunId, attachDraft } from './trace.js'
import { runGates } from './gates.js'
import { composeEnvelope, writeDraft } from './akb-compose.js'
import { canonV1 } from './canon.js'

// Plain fs read (not an import attribute) to stay compatible with Node 18,
// which does not support `import ... with { type: 'json' }`.
const __dirname = dirname(fileURLToPath(import.meta.url))
const policy = JSON.parse(readFileSync(join(__dirname, 'policy.json'), 'utf8'))

function parseArgs(args) {
  const flags = {}
  const positional = []
  for (let i = 0; i < args.length; i++) {
    const a = args[i]
    if (a.startsWith('--')) {
      const name = a.slice(2)
      const next = args[i + 1]
      if (next !== undefined && !next.startsWith('--')) {
        flags[name] = next
        i++
      } else {
        flags[name] = true
      }
    } else {
      positional.push(a)
    }
  }
  return { flags, positional }
}

function rankMode(flags) {
  if (flags.semantic) return 'semantic'
  if (flags.hybrid) return 'hybrid'
  return 'lexical'
}

function cmdSnapshot() {
  const config = getConfig()
  const snapshot = loadSnapshot(config.storePath)
  console.log(JSON.stringify({ corpus_snapshot_hash: snapshot.corpus_snapshot_hash, count: snapshot.count, store_path: config.storePath }, null, 2))
}

function cmdRecall(positional, flags) {
  const query = positional[0]
  if (!query) {
    console.error('Usage: brainmaxx recall "<query>" [--k 12] [--semantic|--hybrid] [--include-stale] [--json]')
    process.exit(1)
  }
  const config = getConfig()
  const snapshot = loadSnapshot(config.storePath)
  const k = flags.k ? Number(flags.k) : DEFAULT_K
  const mode = rankMode(flags)
  const includeStale = Boolean(flags['include-stale'])

  const pack = buildPack(snapshot.entries, query, { k, mode, includeStale })

  if (!pack.items.length) {
    if (flags.json) {
      console.log(JSON.stringify({ result: 'no results above threshold', min_score: MIN_SCORE, pack }, null, 2))
    } else {
      console.log('no results above threshold')
    }
    return
  }

  if (flags.json) {
    console.log(JSON.stringify({ corpus_snapshot_hash: snapshot.corpus_snapshot_hash, pack }, null, 2))
    return
  }

  console.log(`pack_hash: ${pack.pack_hash}  (mode=${pack.mode} k=${pack.k} include_stale=${pack.include_stale})`)
  for (const item of pack.items) {
    console.log(`\n[${item.score}] ${item.moult_id}  (${item.author || 'unknown'}, ${item.mime_type || 'unknown mime'})`)
    console.log(`  ${item.excerpt.replace(/\n/g, ' ')}`)
  }
}

function planInputMarkdown(trace, snapshot) {
  const lines = [
    `# Brainmaxx plan input — ${trace.run_id}`,
    '',
    `**Objective:** ${trace.objective}`,
    `**corpus_snapshot_hash:** ${trace.corpus_snapshot_hash}`,
    `**pack_hash:** ${trace.pack.pack_hash}`,
    '',
    '## Cited sources',
    '',
  ]
  for (const item of trace.pack.items) {
    lines.push(`### ${item.moult_id} (score ${item.score})`)
    lines.push(`- author: ${item.author || 'unknown'}`)
    lines.push(`- mime_type: ${item.mime_type || 'unknown'}`)
    lines.push('')
    lines.push('> ' + item.excerpt.replace(/\n/g, ' '))
    lines.push('')
  }
  lines.push('## Instructions for the D2 (generative) stage')
  lines.push('')
  lines.push('Draft a response grounded ONLY in the sources above. Every factual claim must cite a `moult_id` from this pack. Do not introduce facts not present in the excerpts. Save your draft as plain text/markdown, then run:')
  lines.push('')
  lines.push('```')
  lines.push(`brainmaxx attach ${trace.run_id} <draft-file> [--model <name>]`)
  lines.push('```')
  return lines.join('\n')
}

function cmdPlan(positional, flags) {
  const objective = positional[0]
  if (!objective) {
    console.error('Usage: brainmaxx plan "<objective>" [--k 12]')
    process.exit(1)
  }
  const config = getConfig()
  const snapshot = loadSnapshot(config.storePath)
  const k = flags.k ? Number(flags.k) : DEFAULT_K
  const mode = rankMode(flags)
  const includeStale = Boolean(flags['include-stale'])
  const cfgHash = configHash(config)

  const pack = buildPack(snapshot.entries, objective, { k, mode, includeStale })
  const trace = createTrace({
    mode: 'plan',
    objective,
    corpus_snapshot_hash: snapshot.corpus_snapshot_hash,
    config_hash: cfgHash,
    query_set: [objective],
    pack,
  })

  const gateVerdicts = runGates({
    refs: pack.items.map((i) => i.moult_id),
    claims: [],
    includeStale,
    entries: snapshot.entries,
  })
  trace.gates = gateVerdicts

  const tracePath = writeTrace(config.brainmaxxDir, trace)
  const planInputPath = join(config.brainmaxxDir, 'traces', `${trace.run_id.replace(/:/g, '_')}.PLAN-INPUT.md`)
  writeFileSync(planInputPath, planInputMarkdown(trace, snapshot), 'utf8')

  console.log(`run_id: ${trace.run_id}`)
  console.log(`trace: ${tracePath}`)
  console.log(`plan input: ${planInputPath}`)
  if (!pack.items.length) console.log('no results above threshold — plan input has no cited sources')
}

function cmdAttach(positional, flags) {
  const [runIdArg, draftFile] = positional
  if (!runIdArg || !draftFile) {
    console.error('Usage: brainmaxx attach <run_id> <draft-file> [--model <name>]')
    process.exit(1)
  }
  const config = getConfig()
  const runId = resolveRunId(config.brainmaxxDir, runIdArg)
  const trace = readTrace(config.brainmaxxDir, runId)

  if (!existsSync(draftFile)) {
    console.error(`draft file not found: ${draftFile}`)
    process.exit(1)
  }
  const bytes = readFileSync(draftFile)
  const sha256 = createHash('sha256').update(bytes).digest('hex')

  attachDraft(trace, { path: draftFile, sha256, model: flags.model || 'operator-declared' })
  const tracePath = writeTrace(config.brainmaxxDir, trace)

  console.log(`attached ${draftFile} to ${runId}`)
  console.log(`determinism_profile: ${trace.determinism_profile}`)
  console.log(`trace: ${tracePath}`)
}

function cmdGates(positional) {
  const runIdArg = positional[0]
  if (!runIdArg) {
    console.error('Usage: brainmaxx gates <run_id>')
    process.exit(1)
  }
  const config = getConfig()
  const runId = resolveRunId(config.brainmaxxDir, runIdArg)
  const trace = readTrace(config.brainmaxxDir, runId)
  const snapshot = loadSnapshot(config.storePath)

  const verdicts = runGates({
    refs: trace.pack?.items?.map((i) => i.moult_id) || [],
    claims: trace.claims || [],
    includeStale: trace.pack?.include_stale || false,
    entries: snapshot.entries,
    action: 'gates',
    policy,
  })

  console.log(JSON.stringify(verdicts, null, 2))
  const failed = verdicts.some((v) => v.verdict === 'fail')
  process.exit(failed ? 1 : 0)
}

function cmdMoultDraft(positional) {
  const runIdArg = positional[0]
  if (!runIdArg) {
    console.error('Usage: brainmaxx moult-draft <run_id>')
    process.exit(1)
  }
  const config = getConfig()
  const runId = resolveRunId(config.brainmaxxDir, runIdArg)
  const trace = readTrace(config.brainmaxxDir, runId)
  const snapshot = loadSnapshot(config.storePath)

  let envelope
  try {
    envelope = composeEnvelope(trace, { motherMoultId: config.motherMoultId })
  } catch (e) {
    console.error(`cannot compose envelope: ${e.message}`)
    process.exit(1)
  }
  if (!config.motherMoultId) {
    console.warn('[brainmaxx] warning: MOTHER_MOULT_ID is unset — envelope will omit mother_moult_id')
  }

  const verdicts = runGates({
    refs: envelope.refs,
    claims: trace.claims || [],
    includeStale: trace.pack?.include_stale || false,
    entries: snapshot.entries,
    envelope,
    action: 'moult-draft',
    policy,
  })

  const failed = verdicts.filter((v) => v.verdict === 'fail')
  if (failed.length) {
    console.error('gates failed, draft not emitted:')
    console.error(JSON.stringify(failed, null, 2))
    process.exit(1)
  }

  // Note: moult-draft's envelope-scoped gate verdicts are NOT written back
  // into trace.json — that file's gates reflect the recall/plan context and
  // must stay stable for `brainmaxx replay` (spec §11). The verdicts here
  // are printed for the operator and embedded in the draft's own commitment.
  const { path, commitment } = writeDraft(config.brainmaxxDir, trace, envelope)
  console.log(`gates: ${JSON.stringify(verdicts)}`)
  console.log(`draft: ${path}`)
  console.log(`commitment preview (sha256b64, byte-parity with reply-bot): ${commitment}`)
  console.log('hand this file to tools/reply-bot for human-approved posting')
}

function cmdReplay(positional) {
  const runIdArg = positional[0]
  if (!runIdArg) {
    console.error('Usage: brainmaxx replay <run_id>')
    process.exit(1)
  }
  const config = getConfig()
  const runId = resolveRunId(config.brainmaxxDir, runIdArg)
  const trace = readTrace(config.brainmaxxDir, runId)

  const snapshot = loadSnapshot(config.storePath)
  if (snapshot.corpus_snapshot_hash !== trace.corpus_snapshot_hash) {
    console.error(`corpus moved: recorded ${trace.corpus_snapshot_hash} (n=?), current ${snapshot.corpus_snapshot_hash} (n=${snapshot.count})`)
    process.exit(2)
  }

  const recomputedPack = buildPack(snapshot.entries, trace.query_set[0], {
    k: trace.pack.k,
    mode: trace.pack.mode,
    includeStale: trace.pack.include_stale,
  })

  const recomputedGates = runGates({
    refs: recomputedPack.items.map((i) => i.moult_id),
    claims: trace.claims || [],
    includeStale: trace.pack.include_stale,
    entries: snapshot.entries,
  })

  const recordedPackCanon = canonV1(trace.pack)
  const recomputedPackCanon = canonV1(recomputedPack)
  const recordedGatesCanon = canonV1(trace.gates || [])
  const recomputedGatesCanon = canonV1(recomputedGates)

  if (recordedPackCanon !== recomputedPackCanon) {
    console.error('replay mismatch: pack differs')
    console.error(`  recorded:   ${recordedPackCanon}`)
    console.error(`  recomputed: ${recomputedPackCanon}`)
    process.exit(1)
  }
  if (recordedGatesCanon !== recomputedGatesCanon) {
    console.error('replay mismatch: gates differ')
    console.error(`  recorded:   ${recordedGatesCanon}`)
    console.error(`  recomputed: ${recomputedGatesCanon}`)
    process.exit(1)
  }

  console.log(`replay OK: ${runId} — pack and gates byte-identical`)
  process.exit(0)
}

function main() {
  const [, , cmd, ...rest] = process.argv
  const { flags, positional } = parseArgs(rest)

  switch (cmd) {
    case 'snapshot':
      return cmdSnapshot()
    case 'recall':
      return cmdRecall(positional, flags)
    case 'plan':
      return cmdPlan(positional, flags)
    case 'attach':
      return cmdAttach(positional, flags)
    case 'gates':
      return cmdGates(positional)
    case 'moult-draft':
      return cmdMoultDraft(positional)
    case 'replay':
      return cmdReplay(positional)
    default:
      console.log(`brainmaxx — deterministic D0 cognition CLI over the Reef

Usage:
  brainmaxx snapshot
  brainmaxx recall "<query>" [--k 12] [--semantic|--hybrid] [--include-stale] [--json]
  brainmaxx plan "<objective>" [--k 12]
  brainmaxx attach <run_id> <draft-file> [--model <name>]
  brainmaxx gates <run_id>
  brainmaxx moult-draft <run_id>
  brainmaxx replay <run_id>`)
      process.exit(cmd ? 1 : 0)
  }
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  main()
}

export { parseArgs }
