#!/usr/bin/env node
// Chain Superintelligence Module — J-Lens HTTP service
//
// Exposes the J-Lens audit pipeline as a standalone HTTP server:
//   POST /audit        — single-model audit (extract → probe → attest)
//   POST /panel        — multi-model panel audit (consensus/dissent)
//   GET  /models       — registered model registry (id → bank metadata)
//   GET  /health       — health check
//   GET  /version      — version info
//
// Canonical request (jlens.integrity.v1):
//   POST /audit { "text": "...", "model_id": "qwen2.5-0.5b-instruct" }
//   → resolves endpoint + probe bank from the server-side registry
//     (models.json). Clients NEVER send paths or endpoints.
//
// Legacy request (deprecated, disabled by default):
//   POST /audit { "text": "...", "endpoint": "...", "probe_bank_path": "..." }
//   → only honored when CSI_ALLOW_DIRECT_PATHS=1. Local development
//     escape hatch; never enable on a production listener.
//
// Canonical response adds:
//   integrity          — audited integrity, [0,1], higher = healthier
//   score_semantics    — always "jlens.integrity.v1" (clients verify)
//   model_id, bank_hash, snapshot_hash — chain-anchor metadata
//
// Authentication: Bearer token via CSI_AUTH_TOKEN env var (required).
// Rate limiting: simple in-memory per-IP counter (max 60 req/min).
//
// Usage:
//   CSI_AUTH_TOKEN=$(openssl rand -hex 32) node src/csi-server.js
//   # or: CSI_PORT=8080 CSI_AUTH_TOKEN=secret node src/csi-server.js
//   # custom registry: CSI_MODELS_FILE=/path/models.json

import { createServer } from 'node:http'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'
import { fileURLToPath } from 'node:url'
import { dirname, join, resolve } from 'node:path'

import {
  runFullPipeline,
  runPanelAudit,
  buildPanelAttestation,
  saveAuditReport,
  computeSeparationScore,
  gateVerdict,
  DEFAULT_THRESHOLDS,
  CSI_VERSION,
} from './chain-superintelligence.js'

const PORT = Number(process.env.CSI_PORT) || 0
const HOST = process.env.CSI_HOST || '127.0.0.1'
const AUTH_TOKEN = process.env.CSI_AUTH_TOKEN
const REPORT_DIR = process.env.CSI_REPORT_DIR || null
const MAX_REQ_PER_MIN = 60
const ALLOW_DIRECT_PATHS = process.env.CSI_ALLOW_DIRECT_PATHS === '1'
const SCORE_SEMANTICS = 'jlens.integrity.v1'

if (!AUTH_TOKEN) {
  console.error('CSI_AUTH_TOKEN is required. Generate one with: openssl rand -hex 32')
  process.exit(1)
}

// ─── Model registry ──────────────────────────────────────────────────
// Server-side map: model_id → {endpoint, layer, probe_bank_path, ...}.
// The chain probe-registry contract is authoritative in production;
// this file is the local mirror (jlens.models.v1).

const here = dirname(fileURLToPath(import.meta.url))
const MODELS_FILE = process.env.CSI_MODELS_FILE || join(here, 'models.json')

function loadModelRegistry() {
  const raw = JSON.parse(readFileSync(MODELS_FILE, 'utf8'))
  const baseDir = dirname(MODELS_FILE)
  const registry = new Map()
  for (const [id, m] of Object.entries(raw.models || {})) {
    const bankPath = resolve(baseDir, m.probe_bank_path)
    const bankJson = readFileSync(bankPath, 'utf8')
    const bankHash = createHash('sha256').update(bankJson).digest('hex')
    registry.set(id, {
      model_id: id,
      label: m.label || id,
      endpoint: m.endpoint,
      layer: m.layer ?? -1,
      probe_bank_path: bankPath,
      probe_method: m.probe_method || 'contrastive-dom-v0.1',
      test_only: m.test_only === true,
      bank_hash: bankHash,
    })
  }
  return registry
}

const modelRegistry = loadModelRegistry()
console.error(`[csi] model registry: ${modelRegistry.size} models loaded from ${MODELS_FILE}`)

// Simple in-memory rate limiter
const rateMap = new Map()
function rateLimit(ip) {
  const now = Date.now()
  const entry = rateMap.get(ip) || { count: 0, windowStart: now }
  if (now - entry.windowStart > 60_000) {
    entry.count = 0
    entry.windowStart = now
  }
  entry.count++
  rateMap.set(ip, entry)
  return entry.count <= MAX_REQ_PER_MIN
}

function sendJson(res, status, body) {
  const json = JSON.stringify(body, null, 2)
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(json),
  })
  res.end(json)
}

function authenticate(req) {
  const auth = req.headers.authorization
  if (!auth || !auth.startsWith('Bearer ')) return false
  return auth.slice(7) === AUTH_TOKEN
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    const chunks = []
    let size = 0
    req.on('data', (chunk) => {
      size += chunk.length
      if (size > 1_000_000) {
        reject(new Error('request body too large (max 1MB)'))
        req.destroy()
        return
      }
      chunks.push(chunk)
    })
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')))
    req.on('error', reject)
  })
}

/**
 * Resolve an /audit request body to pipeline opts.
 * Canonical path: {text, model_id} → registry lookup.
 * Legacy path: {text, endpoint, probe_bank_path} → only with flag.
 */
function resolveAuditTarget(body) {
  if (body.model_id) {
    const entry = modelRegistry.get(body.model_id)
    if (!entry) {
      return {
        error: `unknown model_id: ${body.model_id}`,
        hint: 'GET /models lists registered ids',
        status: 400,
      }
    }
    return {
      endpoint: entry.endpoint,
      layer: entry.layer,
      probeBankPath: entry.probe_bank_path,
      entry,
    }
  }

  if (body.endpoint && body.probe_bank_path) {
    if (!ALLOW_DIRECT_PATHS) {
      return {
        error: 'direct endpoint/probe_bank_path is disabled on this server',
        hint: 'send {text, model_id}; model ids resolve server-side. Set CSI_ALLOW_DIRECT_PATHS=1 for local dev only.',
        status: 400,
      }
    }
    return {
      endpoint: body.endpoint,
      layer: body.layer ?? -1,
      probeBankPath: body.probe_bank_path,
      entry: null,
    }
  }

  return { error: 'missing required fields: text, model_id', status: 400 }
}

const server = createServer(async (req, res) => {
  const url = new URL(req.url, `http://${req.headers.host}`)
  const path = url.pathname
  const method = req.method
  const ip = req.socket.remoteAddress

  // CORS
  res.setHeader('Access-Control-Allow-Origin', 'null')
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
  res.setHeader('Access-Control-Allow-Headers', 'Authorization, Content-Type')
  if (method === 'OPTIONS') {
    res.writeHead(204)
    res.end()
    return
  }

  // Health (no auth)
  if (method === 'GET' && path === '/health') {
    sendJson(res, 200, { status: 'ok', version: CSI_VERSION, uptime: process.uptime() })
    return
  }

  // Version (no auth)
  if (method === 'GET' && path === '/version') {
    sendJson(res, 200, {
      version: CSI_VERSION,
      score_semantics: SCORE_SEMANTICS,
      thresholds: DEFAULT_THRESHOLDS,
      endpoints: ['/audit', '/panel', '/models', '/health', '/version'],
    })
    return
  }

  // Models registry (no auth — public metadata only, no paths)
  if (method === 'GET' && path === '/models') {
    sendJson(res, 200, {
      score_semantics: SCORE_SEMANTICS,
      models: [...modelRegistry.values()].map((m) => ({
        model_id: m.model_id,
        label: m.label,
        layer: m.layer,
        probe_method: m.probe_method,
        bank_hash: m.bank_hash,
        test_only: m.test_only,
      })),
    })
    return
  }

  // Auth gate for everything else
  if (!authenticate(req)) {
    sendJson(res, 401, { error: 'unauthorized' })
    return
  }

  // Rate limit
  if (!rateLimit(ip)) {
    sendJson(res, 429, { error: 'rate limit exceeded', limit: `${MAX_REQ_PER_MIN}/min` })
    return
  }

  // POST /audit — single-model audit
  if (method === 'POST' && path === '/audit') {
    try {
      const body = JSON.parse(await readBody(req))
      if (!body.text) {
        sendJson(res, 400, { error: 'missing required field: text' })
        return
      }

      const target = resolveAuditTarget(body)
      if (target.error) {
        sendJson(res, target.status || 400, { error: target.error, hint: target.hint })
        return
      }

      const result = await runFullPipeline({
        endpoint: target.endpoint,
        text: body.text,
        layer: target.layer,
        probeBankPath: target.probeBankPath,
        mode: body.mode || 'dev-sim',
        proposalId: body.proposal_id || 0,
      })

      const integrity = computeSeparationScore(result.snapshot)
      const gate = gateVerdict(integrity)

      sendJson(res, 200, {
        version: CSI_VERSION,
        score_semantics: SCORE_SEMANTICS,
        integrity,
        // deprecated alias — kept for one release, do not consume
        separation_score: integrity,
        gate: gate.gate,
        gate_label: gate.label,
        verdict: result.verdict,
        attestation: result.attestation,
        detections: result.snapshot.detections.length,
        snapshot_hash: result.snapshot.snapshot_hash,
        model_id: target.entry ? target.entry.model_id : null,
        bank_hash: target.entry ? target.entry.bank_hash : null,
      })
    } catch (e) {
      sendJson(res, 500, { error: e.message })
    }
    return
  }

  // POST /panel — multi-model panel audit
  if (method === 'POST' && path === '/panel') {
    try {
      const body = JSON.parse(await readBody(req))
      if (!body.panel || !Array.isArray(body.panel) || !body.panel.length) {
        sendJson(res, 400, { error: 'missing required field: panel (array of model configs)' })
        return
      }
      if (!body.text) {
        sendJson(res, 400, { error: 'missing required field: text' })
        return
      }

      const panelResult = await runPanelAudit({
        panel: body.panel,
        text: body.text,
        hiddenStatesDir: REPORT_DIR || null,
        thresholds: body.thresholds || DEFAULT_THRESHOLDS,
      })

      const attestation = buildPanelAttestation(panelResult, {
        mode: body.mode || 'dev-sim',
        proposalId: body.proposal_id || 0,
      })

      sendJson(res, 200, {
        version: CSI_VERSION,
        score_semantics: SCORE_SEMANTICS,
        panel_verdict: panelResult.panelVerdict,
        consensus: panelResult.consensus,
        models: panelResult.models.map((m) => ({
          model_id: m.modelId,
          integrity: m.separationScore,
          gate: m.gate,
          gate_label: m.gateLabel,
          detections: m.snapshot.detections.length,
          snapshot_hash: m.snapshot.snapshot_hash,
        })),
        attestation,
      })
    } catch (e) {
      sendJson(res, 500, { error: e.message })
    }
    return
  }

  sendJson(res, 404, { error: 'not found', path })
})

server.listen(PORT, HOST, () => {
  const { port } = server.address()
  console.error(`Chain Superintelligence Module v${CSI_VERSION}`)
  console.error(`Listening on http://${HOST}:${port}`)
  console.error(`Endpoints: POST /audit, POST /panel, GET /models, GET /health, GET /version`)
  console.error(`Score semantics: ${SCORE_SEMANTICS} (integrity, higher = healthier)`)
  console.error(`Direct paths: ${ALLOW_DIRECT_PATHS ? 'ENABLED (dev only)' : 'disabled (use model_id)'}`)
  console.error(`Auth: Bearer token (CSI_AUTH_TOKEN)`)
  console.error(`Rate limit: ${MAX_REQ_PER_MIN} req/min per IP`)
})
