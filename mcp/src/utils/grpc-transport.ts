/**
 * Minimal unary gRPC transport for the JunoClaw chain.
 *
 * slay3rd exposes a standard Cosmos SDK gRPC surface on :9090
 * (cosmos.tx.v1beta1.Service, cosmos.bank/auth queries,
 * cosmwasm.wasm queries, layer.lightclient height) — no Tendermint
 * RPC. This module provides the one primitive the adapter needs:
 * a unary request/response call over native HTTP/2 gRPC.
 *
 * Plaintext credentials only (testnet). The endpoint must be an
 * operator-reachable gRPC listener — on this fleet that means an
 * operator IP present in the seat's UFW allow list, or a loopback
 * sidecar. Do not point this at arbitrary public endpoints.
 */

import * as grpc from "@grpc/grpc-js";

const identity = (b: Buffer) => b;

/** One pooled channel per endpoint — grpc-js reuses HTTP/2 streams. */
const clientCache = new Map<string, grpc.Client>();

function getClient(endpoint: string): grpc.Client {
  const cached = clientCache.get(endpoint);
  if (cached) return cached;
  const client = new grpc.Client(
    endpoint,
    grpc.credentials.createInsecure(),
    { "grpc.max_receive_message_length": 64 * 1024 * 1024 }
  );
  clientCache.set(endpoint, client);
  return client;
}

export class GrpcStatusError extends Error {
  constructor(
    public readonly code: number,
    message: string
  ) {
    super(`gRPC ${grpc.status[code] ?? code}: ${message}`);
    this.name = "GrpcStatusError";
  }
}

/**
 * Issue a unary gRPC call. `path` is the fully-qualified method
 * (e.g. "/cosmos.bank.v1beta1.Query/Balance"); request/response are
 * already protobuf-encoded buffers — codecs live in the caller
 * (cosmjs-types for SDK paths, hand-rolled for layer.* paths).
 */
export function grpcUnary(
  endpoint: string,
  path: string,
  request: Buffer,
  deadlineMs = 15_000
): Promise<Buffer> {
  const client = getClient(endpoint);
  return new Promise((resolve, reject) => {
    client.makeUnaryRequest<Buffer, Buffer>(
      path,
      identity,
      identity,
      request,
      { deadline: new Date(Date.now() + deadlineMs) },
      (err, response) => {
        if (err) {
          reject(new GrpcStatusError(err.code ?? grpc.status.UNKNOWN, err.message));
        } else if (response === undefined || response === null) {
          reject(new GrpcStatusError(grpc.status.INTERNAL, "empty response"));
        } else {
          resolve(response);
        }
      }
    );
  });
}
