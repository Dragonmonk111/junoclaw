#!/usr/bin/env node
/**
 * post-message.mjs — Post a kind 1 text note to a Buzz relay channel.
 *
 * Usage:
 *   node tools/akash/post-message.mjs --channel dev --content "Hello from JunoClaw!"
 *
 * Env vars:
 *   BUZZ_PRIVATE_KEY — hex private key (32 bytes)
 *   BUZZ_RELAY_URL   — relay WebSocket URL (wss://... or ws://...)
 */

import WebSocket from "ws";

const PRIVKEY = process.env.BUZZ_PRIVATE_KEY || "2db9dc85c289cf13d42abc9e5d12848c956df73047fab93f5827a3d04119735b";
let RELAY_URL = process.env.BUZZ_RELAY_URL || "wss://buzz.junoclaw.xyz/ws";
RELAY_URL = RELAY_URL.replace(/^http:\/\//, "ws://").replace(/^https:\/\//, "wss://");
if (!RELAY_URL.endsWith("/ws") && !RELAY_URL.includes("/ws?")) RELAY_URL = RELAY_URL.replace(/\/$/, "") + "/ws";

const relayUrl = new URL(RELAY_URL);
const HOST_HEADER = /^\d+\.\d+\.\d+\.\d+$/.test(relayUrl.hostname)
  ? "buzz.junoclaw.xyz"
  : relayUrl.hostname;

// Parse args
const args = process.argv.slice(2);
let channel = "dev";
let content = "";
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--channel" && args[i + 1]) channel = args[i + 1];
  if (args[i] === "--content" && args[i + 1]) content = args[i + 1];
}
if (!content) {
  content = "JunoClaw Buzz relay test message — kind 1 text note posted via post-message.mjs script. The DAO coordination layer is live.";
}

// Signing (same approach as create-channels.mjs)
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
      return JSON.stringify([0, ev.pubkey, ev.created_at, ev.kind, ev.tags, ev.content]);
    }
    signEvent = (event, privkey) => {
      const pubkey = getPubkey(privkey);
      const ev = { ...event, pubkey };
      const id = bytesToHex(sha256(new TextEncoder().encode(serializeEvent(ev))));
      ev.id = id;
      ev.sig = bytesToHex(schnorr.sign(id, privkey));
      return ev;
    };
  } catch {
    console.error("Need nostr-tools or @noble/secp256k1 + @noble/hashes installed.");
    process.exit(1);
  }
}

function createTextNote(channel, content) {
  return {
    kind: 1,
    created_at: Math.floor(Date.now() / 1000),
    tags: [["t", channel]],
    content,
  };
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

async function postMessage(ws, channel, content) {
  return new Promise((resolve, reject) => {
    const event = signEvent(createTextNote(channel, content), PRIVKEY);

    let timeout = setTimeout(() => {
      reject(new Error("Timeout posting message"));
    }, 15000);

    const handler = (data) => {
      try {
        const msg = JSON.parse(data.toString());
        if (msg[0] === "OK" && msg[1] === event.id) {
          clearTimeout(timeout);
          ws.off("message", handler);
          if (msg[2]) {
            console.log(`  ✅ Message accepted (event ${event.id.slice(0, 16)}...)`);
            resolve(event.id);
          } else {
            console.log(`  ❌ Message rejected — ${msg[3] || "no reason"}`);
            reject(new Error(`Message rejected: ${msg[3]}`));
          }
        }
      } catch {}
    };
    ws.on("message", handler);

    console.log(`  Sending kind 1 text note to #${channel}...`);
    ws.send(JSON.stringify(["EVENT", event]));
  });
}

async function main() {
  console.log(`=== Buzz Post Message ===`);
  console.log(`Relay: ${RELAY_URL}`);
  console.log(`Channel: #${channel}`);
  console.log(`Content: ${content.slice(0, 80)}...\n`);

  const ws = new WebSocket(RELAY_URL, {
    headers: { Host: HOST_HEADER },
  });

  await new Promise((resolve, reject) => {
    ws.on("open", resolve);
    ws.on("error", reject);
    setTimeout(() => reject(new Error("Connection timeout")), 10000);
  });

  console.log("Connected to relay.");

  console.log("Authenticating (NIP-42)...");
  try {
    await authenticate(ws);
    console.log("Authenticated.\n");
  } catch (e) {
    console.log(`Auth skipped: ${e.message}\n`);
  }

  try {
    await postMessage(ws, channel, content);
  } catch (e) {
    console.log(`  Error: ${e.message}`);
  }

  ws.close();
  console.log("\nDone.");
}

main().catch(e => {
  console.error(`Fatal: ${e.message}`);
  process.exit(1);
});
