---
title: Fleet & robotics
description: Verifiability for robots — the four witnesses applied to a fleet, sim-to-real attestation, fleet kill-switches and liability anchors.
---

Every digital agent can fail backward: a bad swap reverts, a wrong verdict is disputed, a payment is withheld. **A robot cannot.** A dropped crate, a wrong dose, a blade at the wrong moment — that consequence does not revert. So the proof has to bind **before the actuator moves**, not after settlement.

JunoClaw publishes what the machine was *permitted* to do, and proves it stayed inside that.

## The four witnesses, applied to a fleet

| # | Witness | Question | Status |
|---|---|---|---|
| 1 | TEE attestation | *Who ran what* — a signed measurement that a specific binary ran in a specific enclave on specific silicon | Live |
| 2 | ZK proof (Groth16 via the BN254 precompile) | *Did it behave* — the run satisfied a declared predicate over behaviour, without revealing weights or prompts | Live |
| 3 | Tier-1.5 constraint vocabulary | *What was it allowed to attempt* | Live on `uni-7` |
| 4 | Verifiable controllability | *Is the operator still in control, observably from outside?* | Shipped |

Provenance alone is not enough, and neither is semantics alone. See the [lightpaper](/docs/lightpaper/) for the twin-lock argument.

## What a fleet operator gets

- **Sim-to-real attestation.** Locomotion and manipulation policies are trained in simulation, and the policy artifact is hashed and anchored on-chain — so the attestation binds the exact policy that shipped to the robot to the actions it then took.
- **Compile-time hardening.** Code that should not run is not in the binary at all, so it cannot be reached. `unsafe-shell` today; `unsafe-egress` and `unsafe-fs-write` planned.
- **Runtime kill-switches.** Halt a capability in seconds without a redeploy — single-`curl` mean-time-to-halt. `sandbox_mode` today; `signing_paused` and `egress_paused` planned.
- **A readable policy surface.** A read-only policy RPC is planned so any client can verify which capabilities a fleet has armed *before* sending it a task.
- **Liability anchors.** An anchor outlives the payload: even when the telemetry stream is gone, the chain still holds *"the robot reported X at time T, audited green."*

## Coordination

Fleet channels run on **Buzz**, a DAO-owned Nostr relay at `buzz.junoclaw.xyz` — there is a `#robotics` channel alongside `#governance`, `#truth-market` and `#dev`. The deployment runbook is on the [robotics page](https://junoclaw.xyz/deploy.html).

## Honest status

- **Live:** TEE attestation quote on-chain, Tier-1.5 constraints on `uni-7`, fleet kill-switches and compile-time hardening.
- **Devnet:** sim-to-real robotics attestation.
- **Planned:** read-only fleet policy RPC, and the chain-layer controllability lock.
