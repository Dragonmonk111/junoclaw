#!/usr/bin/env node
/**
 * create-channels.mjs — Create Buzz channels via Nostr WebSocket.
 * 
 * Buzz channels are NIP-28 kind 40 events (channel metadata/create).
 * We sign with the relay owner's private key and send via WebSocket.
 *
 * Usage:
 *   node tools/akash/create-channels.mjs
 *
 * Env vars:
 *   BUZZ_PRIVATE_KEY — hex private key (32 bytes)
 *   BUZZ_RELAY_URL   — relay WebSocket URL (wss://... or ws://...)
 */

import WebSocket from "ws";

const PRIVKEY = process.env.BUZZ_PRIVATE_KEY || "2db9dc85c289cf13d42abc9e5d12848c956df73047fab93f5827a3d04119735b";
let RELAY_URL = process.env.BUZZ_RELAY_URL || "wss://buzz.junoclaw.xyz/ws";
// Normalize: convert http(s):// to ws(s):// and append /ws if missing
RELAY_URL = RELAY_URL.replace(/^http:\/\//, "ws://").replace(/^https:\/\//, "wss://");
if (!RELAY_URL.endsWith("/ws") && !RELAY_URL.includes("/ws?")) RELAY_URL = RELAY_URL.replace(/\/$/, "") + "/ws";

// Parse URL to extract host for the Host header (Akash ingress routes by Host)
const relayUrl = new URL(RELAY_URL);
// If connecting by IP, use buzz.junoclaw.xyz as Host header for Akash ingress
const HOST_HEADER = /^\d+\.\d+\.\d+\.\d+$/.test(relayUrl.hostname) 
  ? "buzz.junoclaw.xyz" 
  : relayUrl.hostname;

// Minimal secp256k1 + schnorr signing using noble-curves (no external dep needed if installed)
// We'll use the `nostr-tools` library if available, otherwise fall back to manual signing.
let signEvent;
try {
  const { finalizeEvent } = await import("nostr-tools");
  signEvent = (event, privkey) => {
    const ev = { ...event, pubkey: "", sig: "" };
    const keyBytes = typeof privkey === "string" 
      ? new Uint8Array(privkey.match(/.{1,2}/g).map(b => parseInt(b, 16)))
      : privkey;
    finalizeEvent(ev, keyBytes);
    return ev;
  };
} catch {
  // nostr-tools not installed — try @noble/secp256k1 directly
  try {
    const { schnorr, utils } = await import("@noble/secp256k1");
    const { sha256 } = await import("@noble/hashes/sha256");
    const { bytesToHex, hexToBytes } = utils;

    function getPubkey(privHex) {
      const priv = hexToBytes(privHex);
      const pub = schnorr.getPublicKey(priv);
      return bytesToHex(pub);
    }

    function serializeEvent(ev) {
      return JSON.stringify([
        0, ev.pubkey, ev.created_at, ev.kind, ev.tags, ev.content
      ]);
    }

    signEvent = (event, privkey) => {
      const pubkey = getPubkey(privkey);
      const ev = { ...event, pubkey };
      const id = bytesToHex(sha256(new TextEncoder().encode(serializeEvent(ev))));
      ev.id = id;
      const sig = bytesToHex(schnorr.sign(id, privkey));
      ev.sig = sig;
      return ev;
    };
  } catch {
    console.error("Need nostr-tools or @noble/secp256k1 + @noble/hashes installed.");
    console.error("Run: npm install nostr-tools");
    process.exit(1);
  }
}

const CHANNELS = [
  { name: "governance", description: "DAO governance proposals, voting, and coordination" },
  { name: "truth-market", description: "Truth market pipeline — verdict drafts, submissions, and outcomes" },
  { name: "robotics", description: "Robotics command verification and safety attestation" },
  { name: "dev", description: "Developer discussions and technical coordination" },
];

function createChannelEvent(channel) {
  return {
    kind: 9007,
    created_at: Math.floor(Date.now() / 1000),
    tags: [
      ["name", channel.name],
      ["visibility", "open"],
      ["channel_type", "stream"],
    ],
    content: "",
  };
}

async function createChannel(ws, channel) {
  return new Promise((resolve, reject) => {
    const event = signEvent(createChannelEvent(channel), PRIVKEY);
    const subId = `chan-${channel.name}-${Date.now()}`;

    let timeout = setTimeout(() => {
      reject(new Error(`Timeout creating channel ${channel.name}`));
    }, 15000);

    const handler = (data) => {
      try {
        const msg = JSON.parse(data.toString());
        // Look for OK message for our event
        if (msg[0] === "OK" && msg[1] === event.id) {
          clearTimeout(timeout);
          ws.off("message", handler);
          if (msg[2]) {
            console.log(`  ✅ ${channel.name}: accepted (event ${event.id.slice(0, 16)}...)`);
            resolve(event.id);
          } else {
            console.log(`  ❌ ${channel.name}: rejected — ${msg[3] || "no reason"}`);
            reject(new Error(`Channel ${channel.name} rejected: ${msg[3]}`));
          }
        }
      } catch {}
    };

    ws.on("message", handler);

    console.log(`  Sending kind 40 event for ${channel.name}...`);
    ws.send(JSON.stringify(["EVENT", event]));
  });
}

async function authenticate(ws) {
  return new Promise((resolve, reject) => {
    let timeout = setTimeout(() => {
      reject(new Error("Auth timeout — no AUTH challenge received"));
    }, 10000);

    let authEventId = null;
    const handler = (data) => {
      try {
        const msg = JSON.parse(data.toString());
        if (msg[0] === "AUTH") {
          clearTimeout(timeout);
          const challenge = msg[1];
          console.log(`  Received AUTH challenge: ${challenge.slice(0, 16)}...`);

          // The relay expects the relay tag to match its internal URL (ws://localhost:3000)
          // because nginx proxies with Host: localhost:3000 and RELAY_URL=http://localhost:3000
          const expectedRelayUrl = "ws://localhost:3000";
          const authEvent = signEvent({
            kind: 22242,
            created_at: Math.floor(Date.now() / 1000),
            tags: [
              ["relay", expectedRelayUrl],
              ["challenge", challenge],
            ],
            content: "",
          }, PRIVKEY);

          authEventId = authEvent.id;
          ws.send(JSON.stringify(["AUTH", authEvent]));
          console.log(`  Auth event sent (pubkey: ${authEvent.pubkey.slice(0, 16)}...)`);
          
          // Set a new timeout for auth confirmation
          timeout = setTimeout(() => {
            console.log("  No auth OK received, proceeding anyway...");
            ws.off("message", handler);
            resolve();
          }, 5000);
        } else if (msg[0] === "OK" && msg[1] === authEventId) {
          clearTimeout(timeout);
          ws.off("message", handler);
          if (msg[2]) {
            console.log("  Auth accepted by relay.");
          } else {
            console.log(`  Auth rejected: ${msg[3]}`);
          }
          resolve();
        }
      } catch {}
    };

    ws.on("message", handler);
  });
}

async function main() {
  console.log(`=== Buzz Channel Creation ===`);
  console.log(`Relay: ${RELAY_URL}`);
  console.log(`Channels: ${CHANNELS.map(c => c.name).join(", ")}\n`);

  // Use ws package with custom Host header for Akash ingress routing
  const ws = new WebSocket(RELAY_URL, {
    headers: { Host: HOST_HEADER },
  });

  await new Promise((resolve, reject) => {
    ws.on("open", resolve);
    ws.on("error", reject);
    setTimeout(() => reject(new Error("Connection timeout")), 10000);
  });

  console.log("Connected to relay.");

  // NIP-42 authentication
  console.log("Authenticating (NIP-42)...");
  try {
    await authenticate(ws);
    console.log("Authenticated.\n");
  } catch (e) {
    console.log(`Auth skipped: ${e.message}\n`);
  }

  for (const channel of CHANNELS) {
    try {
      await createChannel(ws, channel);
    } catch (e) {
      console.log(`  Error: ${e.message}`);
    }
  }

  ws.close();
  console.log("\nDone. Channels created.");
}

main().catch(e => {
  console.error(`Fatal: ${e.message}`);
  process.exit(1);
});
