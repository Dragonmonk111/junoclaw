#!/usr/bin/env node
// Chain Superintelligence Module — HTTP service
//
// Exposes the J-Lens audit pipeline as a standalone HTTP server:
//   POST /audit        — single-model audit (extract → probe → attest)
//   POST /panel        — multi-model panel audit (consensus/dissent)
//   GET  /health       — health check
//   GET  /version      — version info
//
// Authentication: Bearer token via CSI_AUTH_TOKEN env var (required).
// Rate limiting: simple in-memory per-IP counter (max 60 req/min).
//
// Usage:
//   CSI_AUTH_TOKEN=$(openssl rand -hex 32) node src/csi-server.js
//   # or with env: CSI_PORT=8080 CSI_AUTH_TOKEN=secret node src/csi-server.js

import { createServer } from 'node:http'
import { readFileSync } from 'node:fs'
import { createHash } from 'node:crypto'

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

if (!AUTH_TOKEN) {
  console.error('CSI_AUTH_TOKEN is required. Generate one with: openssl rand -hex 32')
  process.exit(1)
}

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
      thresholds: DEFAULT_THRESHOLDS,
      endpoints: ['/audit', '/panel', '/health', '/version'],
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
      if (!body.endpoint || !body.text || !body.probe_bank_path) {
        sendJson(res, 400, { error: 'missing required fields: endpoint, text, probe_bank_path' })
        return
      }

      const result = await runFullPipeline({
        endpoint: body.endpoint,
        text: body.text,
        layer: body.layer ?? -1,
        probeBankPath: body.probe_bank_path,
        mode: body.mode || 'dev-sim',
        proposalId: body.proposal_id || 0,
      })

      const sepScore = computeSeparationScore(result.snapshot)
      const gate = gateVerdict(sepScore)

      sendJson(res, 200, {
        version: CSI_VERSION,
        separation_score: sepScore,
        gate: gate.gate,
        gate_label: gate.label,
        verdict: result.verdict,
        attestation: result.attestation,
        detections: result.snapshot.detections.length,
        snapshot_hash: result.snapshot.snapshot_hash,
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
        panel_verdict: panelResult.panelVerdict,
        consensus: panelResult.consensus,
        models: panelResult.models.map((m) => ({
          model_id: m.modelId,
          separation_score: m.separationScore,
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
  console.error(`Endpoints: POST /audit, POST /panel, GET /health, GET /version`)
  console.error(`Auth: Bearer token (CSI_AUTH_TOKEN)`)
  console.error(`Rate limit: ${MAX_REQ_PER_MIN} req/min per IP`)
})
