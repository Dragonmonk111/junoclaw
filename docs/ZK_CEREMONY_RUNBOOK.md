# ZK Ceremony Runbook — jlens-readout production VK

Status: PLANNED. Replaces the fixture `circuit_specific_setup` key with a
sequential multi-party contribution across the validator fleet. Until the
production VK lands, every on-chain Groth16 verification is **fixture-key
only** — honest for demos, not for production trust.

## What we're proving and why a ceremony

Circuit: `circuits/jlens-readout` (`JLensReadoutCircuit`), Groth16/BN254,
5 public inputs `[merkle_root, bank_commitment, score, integrity,
detection]`, canonical `jlens.integrity.v1` leaf binding.

Groth16 needs a circuit-specific structured reference string (SRS). The
SRS embeds "toxic waste" — if *every* contributor retained and pooled
their secret, they could forge proofs for false readouts. Security
assumption: **at least one honest contributor destroys their entropy.**
N independent validator seats make that assumption cheap.

## Phase 0 — freeze the circuit (do this first)

The SRS is bound to the exact R1CS. Any circuit edit after contribution
invalidates the key.

1. Tag the freeze commit in `circuits/jlens-readout/src/lib.rs`.
2. Record the constraint count + circuit source hash:

   ```
   cargo test -p jlens-readout -- --nocapture dump_r1cs_hash
   ```

   (add this test — serializes the ConstraintSystem and sha256s it;
   becomes the `circuit_hash` every contributor re-verifies.)
3. Publish: freeze commit sha, `circuit_hash`, `DIM`, `SCALE`, public
   input order. This is the ceremony manifest.

### FROZEN MANIFEST — v1 (recorded)

```
circuit        jlens-readout (JLensReadoutCircuit), Groth16/BN254
DIM            8
SCALE          1_000_000
public inputs  [merkle_root, bank_commitment, score, integrity, detection]
constraints    5423
instance vars  6
witness vars   5422
r1cs sha256    117e96f13bc4b23a85a1d572728a12af19c4eab8498a3e1552d8039fe78dc96e
reproduce      cargo test -p jlens-readout-circuit dump_r1cs_hash -- --nocapture
```

Any change to `circuits/jlens-readout` (or the shared
`sensor-safety` Merkle/MiMC gadgets it imports) that alters this digest
means a new ceremony — verify the digest at every contribution.

## Phase 1 — the contribution tool (BUILT)

`circuits/zk-ceremony` — single bin, arkworks 0.5, delta-scaling MPC
(BGM17-style). No external ceremony deps; the math is ~80 lines.

```
zk-ceremony init <out.bin>
zk-ceremony contribute <in.bin> <out.bin> <seat> <attestation>
zk-ceremony verify-pair <before.bin> <after.bin>
zk-ceremony finalize <params.bin> <out_dir>
zk-ceremony prove <params.bin> <out_dir>
```

- `init` — base SRS via `circuit_specific_setup` under a PUBLIC
  deterministic seed (`sha256("junoclaw-jlens-readout-v1-ceremony-base")`
  → ChaCha20Rng). Reproducible by anyone; the coordinator contributes
  zero trust — all safety comes from the contribution chain.
- `contribute` — fresh ρ from OsRng (mixed with seat+attestation).
  δ scales by ρ on both groups; h/l queries by ρ⁻¹. Emits hash-linked
  before/after digests. ρ lives in process memory only — nothing
  secret reaches the transcript file.
- `verify-pair` — checks a contribution is exactly "old SRS × one
  consistent ρ": published ρ_G1/ρ_G2 match via pairing, all non-δ
  elements byte-identical, δ-pair consistency, every h/l query
  rescaled by ρ⁻¹ (~52s in release for the 5.4k-element SRS — a
  one-time cost per hand-off).
- `finalize` — extracts the production VK (`jlens-vk-prod.b64`,
  arkworks-compressed, same format the contract stores) + prover key,
  then SELF-TESTS: proves the public fixture witness and verifies it
  against the final VK. Refuses to finalize an un-contributed SRS.
- `prove` — emits `jlens-proof-prod.b64` + `jlens-inputs-prod.b64`
  (160 bytes, 5 Fr) from the transcript PK — the artifacts
  `resolve_with_proof` consumes for the post-swap smoke test.

Dry-run result (2-seat local rehearsal): both transitions verified,
self-test proof passed against the production VK.

Entropy handling (non-negotiable):

- Contributor entropy (ρ) lives only in process memory; the only disk
  output is the transformed `params_i.bin`, which contains NO secret
  material — it is public audit data (seat name, published [ρ]G1/[ρ]G2,
  attestation string).
- Operators may additionally mix a passphrase into the entropy seed
  (defense against a weak host CSPRNG); it never leaves the process.
- After the final contribution, each operator attests destruction of
  their entropy in the attestation string itself.

## Phase 2 — the fleet ceremony

Sequential, ordered, one contribution per seat. Roster = validator set
(post-reshare seats only — seats with live share material).

For each seat i:

1. Coordinator sends `params_{i-1}.bin` + expected `params_before_hash`.
2. Seat verifies hash, runs `contribute params_{i-1}.bin params_i.bin
   <seat-name> "<attestation>"`, returns `params_i.bin` (the
   contribution record is embedded in the file).
3. Coordinator runs `verify-pair params_{i-1}.bin params_i.bin`.
   On failure: seat re-runs or is skipped — one bad contribution does
   not abort the ceremony (as long as ≥1 honest contribution lands,
   the SRS is safe).
4. Anchor the `params_i` sha256 on thought-court as a ceremony
   attestation (`anchor_attestation`, `id=ceremony-contrib-{i}`) so the
   transcript chain is timestamped on the chain it will serve.

File transfer discipline — same rules as DKG ceremonies:

- Named files only, never `scp dir/*`.
- Tarball per hand-off with sha256 manifest; verify recipient-side.
- No HTTP-serving any directory containing ceremony working files.
- `params_*.bin` is NOT secret (safe to transfer in the open); the
  secret is the contributor's entropy, which never leaves the process.

## Phase 3 — finalize + VK swap

1. After the last seat: `zk-ceremony finalize` → `final_params.bin`,
   extract `VerifyingKey` (arkworks canonical serialization, b64).
2. Beacon check (optional but cheap): one extra contribution derived
   from a *future drand round* — proves no contributor knew the final
   SRS structure in advance. We already verify drand sigs on-chain;
   same primitive, off-chain use.
3. Hash the final VK → anchor `zk-vk-production` on thought-court with
   the ceremony transcript hash in `activation_root` field (or a
   dedicated attestation id). The chain now carries the binding:
   transcript → VK.
4. `StoreVk` on a fresh zk-verifier instance (admin path, one VK per
   instance — deploy new rather than mutating the fixture contract,
   keeping the demo path reproducible). Re-point the court via
   `SetZkVerifier`.
5. `zk-ceremony prove final_params.bin out/` → submit the emitted
   proof+inputs to `resolve_with_proof` on a fresh attestation →
   `finalized`. That is the end-to-end production receipt.

## Rollback / failure modes

- Ceremony stalls mid-roster → any prefix of contributions is valid;
  resume from last verified `params_i.bin`.
- Final VK fails an on-chain verify → do NOT swap; keep fixture VK,
  re-run ceremony. `StoreVk` is admin-gated and one-shot per instance;
  a bad swap can't happen silently — a wrong key just fails every
  proof, it never verifies a false one.
- Circuit edit needed after freeze → new ceremony. Treat v1 as frozen
  unless a correctness bug is found.

## Honest limits (say this out loud)

- Groth16 toxic-waste assumption: ≥1 honest seat. Fleet size makes it
  strong but it is still an assumption — the recompute tier stays the
  zero-assumption path.
- This is a small sequential ceremony (N ≤ ~11 seats). Fine for a
  circuit this size; larger circuits would want perpetual-powers-of-tau
  style tooling.
