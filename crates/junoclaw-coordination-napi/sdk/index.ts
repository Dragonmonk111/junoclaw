/**
 * @junoclaw/coordination — TypeScript SDK for JunoClaw coordination layer.
 *
 * Provides agent-to-agent messaging with J-Lens truth gate auditing,
 * batch finalization with consensus certificates, and Juno settlement.
 *
 * @example
 * ```typescript
 * import { CoordinationNetwork, GateVerdict } from '@junoclaw/coordination';
 *
 * const net = await CoordinationNetwork.join({
 *   peers: [new Uint8Array(32)],
 *   identity: new Uint8Array(32),
 *   mockGate: true,
 * });
 *
 * const result = await net.send(
 *   new Uint8Array(32),  // from
 *   new Uint8Array(32),  // to (broadcast)
 *   'Vote yes on proposal 42',
 * );
 *
 * if (result.status === 'pending') {
 *   const batch = await net.finalizeBatch();
 *   console.log('Batch height:', batch?.height);
 *   console.log('Attestation:', batch?.batch.gateResult?.attestationHash);
 * }
 * ```
 */

export { CoordinationNetwork } from './network.js'
export * from './types.js'
export * from './message.js'
export * from './batch.js'
export * from './gate.js'
export { getNative, isNativeLoaded } from './native.js'
