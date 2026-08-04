#!/usr/bin/env node
// Domain-General Audit API — standalone HTTP service
//
// Wraps the J-Lens pipeline for any domain (medical, legal, financial,
// scientific, etc.). Routes text through the same five-stage pipeline:
//   deploy -> probe -> attest -> settle -> gate
//
// Endpoints:
//   POST /audit          — submit text for domain-specific AI integrity audit
//   GET  /domains        — list supported domains and their probe banks
//   GET  /attestations   — list recent attestations (in-memory)
//   GET  /attestation/:id — get a specific attestation by hash
//   GET  /health         — health check
//   GET  /version        — version info
//
// Authentication: Bearer token via AUDIT_API_TOKEN env var (required).
//
// Usage:
//   AUDIT_API_TOKEN=$(openssl rand -hex 32) node src/audit-api.js
//
// Domain probe banks are loaded from a directory specified by
// AUDIT_PROBE_DIR env var (default: ./probe-banks/). Each domain has:
//   <domain>.probe_bank.json  — J-Lens probe bank for that domain
//   <domain>.meta.json        — { label, description, thresholds? }

import { createServer } from 'node:http'
import { readFileSync, existsSync, readdirSync, mkdirSync } from 'node:fs'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createHash } from 'node:crypto'

import {
  runFullPipeline,
  saveAuditReport,
  computeSeparationScore,
  gateVerdict,
  DEFAULT_THRESHOLDS,
  CSI_VERSION,
} from './chain-superintelligence.js'

const __dirname = dirname(fileURLToPath(import.meta.url))

const PORT = Number(process.env.AUDIT_API_PORT) || 0
const HOST = process.env.AUDIT_API_HOST || '127.0.0.1'
const AUTH_TOKEN = process.env.AUDIT_API_TOKEN
const PROBE_DIR = process.env.AUDIT_PROBE_DIR || join(__dirname, '..', 'probe-banks')
const REPORT_DIR = process.env.AUDIT_REPORT_DIR || null
const MAX_REQ_PER_MIN = 30
const MAX_TEXT_LEN = 100_000

if (!AUTH_TOKEN) {
  console.error('AUDIT_API_TOKEN is required. Generate one with: openssl rand -hex 32')
  process.exit(1)
}

// Load domain registry
function loadDomains() {
  const domains = {}
  if (!existsSync(PROBE_DIR)) {
    mkdirSync(PROBE_DIR, { recursive: true })
    return domains
  }
  const files = readdirSync(PROBE_DIR)
  for (const f of files) {
    if (f.endsWith('.probe_bank.json')) {
      const domain = f.replace('.probe_bank.json', '')
      const bankPath = join(PROBE_DIR, f)
      const metaPath = join(PROBE_DIR, `${domain}.meta.json`)
      let meta = { label: domain, description: '' }
      if (existsSync(metaPath)) {
        try { meta = JSON.parse(readFileSync(metaPath, 'utf8')) } catch {}
      }
      domains[domain] = { ...meta, probeBankPath: bankPath }
    }
  }
  return domains
}

const DOMAINS = loadDomains()

// In-memory attestation store (for demo/dev; production uses on-chain)
const attestationStore = new Map()

// Rate limiter
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

  res.setHeader('Access-Control-Allow-Origin', 'null')
  res.setHeader('Access-Control-Allow-Methods', 'GET, POST, OPTIONS')
  res.setHeader('Access-Control-Allow-Headers', 'Authorization, Content-Type')
  if (method === 'OPTIONS') {
    res.writeHead(204)
    res.end()
    return
  }

  // Public endpoints
  if (method === 'GET' && path === '/health') {
    sendJson(res, 200, {
      status: 'ok',
      version: CSI_VERSION,
      domains: Object.keys(DOMAINS).length,
      uptime: process.uptime(),
    })
    return
  }

  if (method === 'GET' && path === '/version') {
    sendJson(res, 200, {
      version: CSI_VERSION,
      product: 'domain-general-audit-api',
      pipeline: ['deploy', 'probe', 'attest', 'settle', 'gate'],
      thresholds: DEFAULT_THRESHOLDS,
    })
    return
  }

  if (method === 'GET' && path === '/domains') {
    const listing = {}
    for (const [key, val] of Object.entries(DOMAINS)) {
      listing[key] = {
        label: val.label,
        description: val.description,
        thresholds: val.thresholds || DEFAULT_THRESHOLDS,
      }
    }
    sendJson(res, 200, { domains: listing })
    return
  }

  // Auth gate
  if (!authenticate(req)) {
    sendJson(res, 401, { error: 'unauthorized' })
    return
  }

  if (!rateLimit(ip)) {
    sendJson(res, 429, { error: 'rate limit exceeded', limit: `${MAX_REQ_PER_MIN}/min` })
    return
  }

  // GET /attestations — list recent
  if (method === 'GET' && path === '/attestations') {
    const list = [...attestationStore.values()].slice(-50).reverse()
    sendJson(res, 200, { attestations: list, count: list.length })
    return
  }

  // GET /attestation/:id
  const attMatch = path.match(/^\/attestation\/([a-f0-9]+)$/)
  if (method === 'GET' && attMatch) {
    const id = attMatch[1]
    const att = attestationStore.get(id)
    if (!att) {
      sendJson(res, 404, { error: 'attestation not found', id })
      return
    }
    sendJson(res, 200, att)
    return
  }

  // POST /audit — the main endpoint
  if (method === 'POST' && path === '/audit') {
    try {
      const body = JSON.parse(await readBody(req))

      if (!body.text) {
        sendJson(res, 400, { error: 'missing required field: text' })
        return
      }
      if (!body.domain) {
        sendJson(res, 400, { error: 'missing required field: domain (use GET /domains to list)' })
        return
      }
      if (!body.endpoint) {
        sendJson(res, 400, { error: 'missing required field: endpoint (Akash GPU URL for hidden states extraction)' })
        return
      }

      if (body.text.length > MAX_TEXT_LEN) {
        sendJson(res, 400, { error: `text too long (max ${MAX_TEXT_LEN} chars)`, length: body.text.length })
        return
      }

      const domain = DOMAINS[body.domain]
      if (!domain) {
        sendJson(res, 400, {
          error: `unknown domain: ${body.domain}`,
          available: Object.keys(DOMAINS),
        })
        return
      }

      const thresholds = domain.thresholds || DEFAULT_THRESHOLDS
      const layer = body.layer ?? -1
      const mode = body.mode || 'dev-sim'
      const proposalId = body.proposal_id || 0

      const result = await runFullPipeline({
        endpoint: body.endpoint,
        text: body.text,
        layer,
        probeBankPath: domain.probeBankPath,
        mode,
        proposalId,
      })

      const sepScore = computeSeparationScore(result.snapshot)
      const gate = gateVerdict(sepScore, thresholds)

      const auditId = createHash('sha256')
        .update(result.attestation.attestation_hash)
        .digest('hex')
        .slice(0, 16)

      const response = {
        audit_id: auditId,
        version: CSI_VERSION,
        domain: body.domain,
        domain_label: domain.label,
        timestamp: new Date().toISOString(),
        input_text_hash: createHash('sha256').update(body.text).digest('hex'),
        input_text_length: body.text.length,
        separation_score: Number(sepScore.toFixed(6)),
        gate: gate.gate,
        gate_label: gate.label,
        verdict: result.verdict,
        detections: result.snapshot.detections.map((d) => ({
          concept: d.concept,
          token: d.token,
          position: d.position,
          score: d.jacobian_score,
          threshold: d.threshold,
        })),
        detections_count: result.snapshot.detections.length,
        snapshot_hash: result.snapshot.snapshot_hash,
        attestation: result.attestation,
        pipeline_stages: {
          deploy: 'complete',
          probe: 'complete',
          attest: 'complete',
          settle: result.txResult ? 'complete' : 'pending (no on-chain client)',
          gate: gate.gate,
        },
      }

      attestationStore.set(result.attestation.attestation_hash, response)

      if (REPORT_DIR) {
        const reportPath = join(REPORT_DIR, `audit-${auditId}.json`)
        saveAuditReport(reportPath, {
          snapshot: result.snapshot,
          verdict: result.verdict,
          attestation: result.attestation,
          txResult: result.txResult,
          text: body.text,
          endpoint: body.endpoint,
          mode,
        })
        response.report_path = reportPath
      }

      const httpStatus = gate.gate === 'red' ? 403 : 200
      sendJson(res, httpStatus, response)
    } catch (e) {
      sendJson(res, 500, { error: e.message })
    }
    return
  }

  sendJson(res, 404, { error: 'not found', path })
})

server.listen(PORT, HOST, () => {
  const { port } = server.address()
  console.error(`Domain-General Audit API v${CSI_VERSION}`)
  console.error(`Listening on http://${HOST}:${port}`)
  console.error(`Probe banks: ${PROBE_DIR}`)
  console.error(`Domains: ${Object.keys(DOMAINS).join(', ') || '(none — add .probe_bank.json files to probe-banks/)'}`)
  console.error(`Endpoints: POST /audit, GET /domains, GET /attestations, GET /attestation/:id, GET /health, GET /version`)
  console.error(`Auth: Bearer token (AUDIT_API_TOKEN)`)
  console.error(`Rate limit: ${MAX_REQ_PER_MIN} req/min per IP`)
})
