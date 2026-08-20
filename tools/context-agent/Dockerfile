# Dockerfile for the JunoClaw context agent.
#
# The context agent is a read-only Node.js service that indexes Moultbook
# heartbeat entries and serves them over HTTP. It needs:
#   - Node.js 20+ (ES modules, top-level await in cosmjs)
#   - @cosmjs/cosmwasm-stargate + @cosmjs/stargate (npm install)
#   - cache/ directory for the index (persisted via volume)
#   - mother-moult.json (seed data, copied into image)
#
# Runtime env vars:
#   PORT                  (default 3000) — HTTP listen port
#   JUNO_RPC_ENDPOINT     — Juno RPC URL (e.g. http://localhost:26657)
#   JUNO_REST_ENDPOINT    — Juno REST/LCD URL
#   MOULTBOOK_ADDR        — Moultbook-v0 contract address
#   HEARTBEAT_AUTHOR      — Wallet to crawl for heartbeat entries
#   EXTRA_SEED_AUTHORS    — Comma-separated additional wallet addresses
#   REFRESH_INTERVAL_MS   — Index refresh interval (default 300000 = 5min)
#   CACHE_DIR             — Index cache directory (default /app/cache)
#   REDMARK_MIN_TRUST_SCORE — Minimum trust score for redmarks (default 10)
#   DAO_CORE              — DAO core contract address
#   PROPOSAL_MODULE       — Proposal module contract address

FROM node:20-slim AS base

WORKDIR /app

# Install production dependencies only
COPY package.json package-lock.json ./
RUN npm ci --omit=dev

# Copy application source + seed data
COPY src/ ./src/
COPY mother-moult.json ./

# Create cache directory (mount a volume here in production)
RUN mkdir -p /app/cache

# Environment defaults
ENV PORT=3000
ENV CACHE_DIR=/app/cache
ENV REFRESH_INTERVAL_MS=300000
ENV NODE_ENV=production

# Expose the HTTP port
EXPOSE 3000

# Health check — the /health endpoint returns 200
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
  CMD node -e "fetch('http://localhost:'+(process.env.PORT||3000)+'/health').then(r=>process.exit(r.ok?0:1)).catch(()=>process.exit(1))"

# Run the context agent
CMD ["node", "src/index.js"]
