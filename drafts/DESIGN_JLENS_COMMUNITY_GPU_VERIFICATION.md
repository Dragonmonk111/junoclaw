# J-Lens on community GPUs — verifiable inference without hardware attestation

Status: draft v1. Supersedes the "TEE as trust root" assumption. The
previous direction (WAVS inside Intel TDX / cloud confidential VMs) is
demoted to an optional accelerator tier, for the reason below.

## 1. Hardware reality

GPU confidential computing (attested compute over the GPU itself) exists
only on NVIDIA Hopper/Blackwell data-centre parts (H100, H200, B200,
RTX PRO 6000 BSE) under a TDX or SEV-SNP host. No consumer card — RTX
30xx/40xx/50xx — ships with attestation, and none will: it is a silicon
and VBIOS feature tier. Intel TDX and AMD SEV-SNP protect the host VM,
not the consumer GPU that does the forward pass.

So "verifiable inference on community GPUs" cannot mean "run the model
inside an enclave". It must mean: **verify the forward pass by
cryptographic commitment plus independent recompute, with validator
stake as the collateral**. That is what this doc specifies.

## 2. Threat model

Operator O runs model M (open weights, pinned by content
`weights_hash`), generates response R to prompt P, captures layer-L
activations for every token position, and commits
`activation_root = merkle(int8(vector(p)))` per position in the sealed
snapshot. What O could lie about:

- (a) **Wrong model.** Run different weights. Caught cheaply by the
  response itself (anyone can regenerate), and formally by recompute.
- (b) **Fabricated activations.** Publish vectors that were never
  computed so the committed detection set clears the gate (e.g. launder
  a red into green). The binding question this doc answers.
- (c) **Selective censoring.** Run honestly but replace the vectors at
  detected positions.
- (d) **Data withholding.** Commit, then refuse to disclose the
  activation blobs during a challenge.

To clear the bank's gate an attacker must ensure *no position* crosses
*any* concept threshold, i.e. true detections force tampering on the
entire detected set (typically several positions across spans), not one
edit. (See §6 for why this matters.)

## 3. Substrate that already exists (live on junoclaw-g1)

- content-pinned `weights_hash` (sha256 over weight-file bytes)
- per-position int8 quanized activation leaves + Merkle root
- `merkle-verifier` contract (anchor + on-chain proof, code 22)
- `probe-registry` contract (bank registry keyed by
  model/revision/weights, code 23; calibration-gated activation)
- offline re-audit tool (`verify_thought.py`)
- published retention bundles for anchorable disclosure

## 4. Protocol — optimistic activation verification

4.1 **Commit.** Snapshot + root anchored at batch height h. Disclosure
payload (prompt text, response *token ids*, activation blobs) retrievable
for the challenge window W blocks. `response_hash` binds ids via
`decode(ids) == R`; `prompt_hash` binds `P`.

4.2 **Challenge window.** For W blocks, any bonded party may challenge
a batch. While unchallenged after W, the batch is provisionally final
for gated classes (provisional acceptance: see 4.6).

4.3 **Committee assignment.** A challenge derives a deterministic seed
`s = H(merkle_root ‖ h ‖ challenger_addr)`; k of m registered verifier
seats are selected with seat-weight sampling. Seats are validator seats
or bonded sub-stakers (§7). Assignment is unknowable at anchor time.

4.4 **Recompute.** Each selected verifier independently runs the
teacher-forced reference harness (§5): one forward pass over
`P ‖ ids(R)`, extract layer L at every absolute position, quantize to
int8, and compare against the committed leaves under the int8 tolerance
rules. One pass covers *all* positions — no per-position sampling is
needed (see §6).

4.5 **Verdict and evidence.** Verifiers sign
`H(batch ‖ recompute_digest ‖ verdict)`; the dispute contract accepts a
quorum (⌈2k/3⌉) verdict. Fraud proof packaging: contested position i,
its committed leaf + Merkle path vs `activation_root`, and the quorum
attestation of the recomputed vector. Slash `S_op` from the operator's
bond on a fraudulent verdict; slash `S_ch` from a false accuser.
Availability failure (4.1 payload undeliverable after deadline): batch
rejected for gated classes + partial bond forfeit.

4.6 **Provisional quorum (fast path).** For latency-sensitive work,
κ operators each produce their own committed snapshot of the same
prompt; the batch is accepted when κ agree on the *detection set*
(integer equality, tolerant to float noise) plus − clarify − the dispute
window still applies after the fact.

## 5. Reference harness — the "and then code" artifact

`tools/brainmaxx/jlens-serve/challenge_recompute.py`:

1. Re-tokenize prompt; check `prompt_hash`.
2. Decode published `response_ids`; check `response_hash`.
3. One teacher-forced forward of the expanded sequence up to layer L;
   capture every position (no `generate`, no KV cache).
4. Quantize each position with the same int8 rule; compare per-position
   against committed leaves and report max divergence.
5. Rebuild detection set / integrity / gate from recomputed vectors and
   compare to the snapshot; PASS iff all match within tolerance.

Adversarial modes for harness self-test: `--tamper i` rewrites a leaf
(must FAIL); `--launder` replaces detection positions with a benign
position's bytes (must FAIL).

## 6. Soundness math

Because one teacher-forced pass covers every position, soundness is not
a per-position sampling problem; it is a **per-batch challenge
probability** problem plus economics.

Let f = probability a batch is challenged (protocol parameter; enforced
by seed + incentives), and suppose an operator launders t batches.

- Evasion probability: `P(evade all) = (1 − f)^t`
- Caught at least once: `P = 1 − (1 − f)^t`

Required f for 99% detection:

| t (laundered batches) | f needed |
|---|---|
| 1   | 99.0% |
| 5   | 60.2% |
| 10  | 37.0% |
| 25  | 16.8% |
| 100 | 4.50% |

So for persistent laundering, modest f suffices. For a single laundered
batch, probability alone cannot work — **bond economics** must price it:
rational deterrence needs expected loss > gain, i.e.
`f · S_op > V`, where V is the value of a successful laundering
(fee harvested, gated action approved). Example: f = 20%, V = 100
JCLAW ⇒ S_op ≥ 500 JCLAW. Bond sizing is a governance parameter per
action class; the *class ceiling* on gated-value must not exceed
`f · S_op`.

Verifier cost: teacher-forced recompute of a batch costs roughly
`(L/d) ×` one full-sequence forward ≈ 30–50% of a generation for
Qwen-scale models, *with no autoregressive loop* (all positions in
parallel). Committee of k, challenge rate f ⇒ total verification load
≈ `f · k · 0.4 ×` generation load. At f = 20%, k = 3: ~24% overhead.

Committee honesty: with m verifier seats and seat weight uniformly
honest ≥ ⌈2m/3⌉, a corrupt-majority committee requires adversarial
control of ≥ ⌈2k/3⌉ of the sampled seats; honest-majority assumptions
match the chain's own N3f1 model, and seats are bond-backed (§7).
Float-nondeterminism false-accusation risk is governed by the measured
quantize tolerance; see Appendix A for the empirical drift measurement
that fixes the tolerance parameter.

### 6.1 Where this lands against zkML

| | this design | zkML today | TEE accelerator |
|---|---|---|---|
| trust root | economics + public recompute | cryptographic soundness | silicon |
| prover cost | ~0 (operator) / 0.4× per verifier | orders of magnitude per token; multi-minute-hours prove time for GPT-scale today | enclave overhead only |
| latency | provisional in seconds; final after window | minutes-hours | seconds |
| runs on consumer GPU | yes | prover needs datacenter-class time | no (no attestation) |
| production-ready | with court contract + committee | not for generative models at our sizes | with quote-verification work |

Position: optimistic verification ships now; zkML is tracked as
research (a ZK-provable forward pass would collapse the challenge
window to zero); TEE sets become an *accelerator tier* whose attestation
counts as boosted evidence, never the root.

## 7. Stake-bound verification (“zk-TEE around chain security”)

The value a validator holds is tied to proof generation: operator and
verifier duties require a seat bond, and attestation fraud slashes it.

- Every attesting operator stakes `S_op` in the dispute contract;
  weighted gated classes require `S_op ≥ class_value / f`.
- Verifier seats are drawn from validator seats (who already carry a
  bond) plus bonded non-validator operators. A false accusation
  (committee falsifiable by full recompute in escalation) burns `S_ch`
  ≥ committee recompute cost × safety factor.
- Escrow-bond design preferred over consensus-level slashing for v1
  (keeps consensus code untouched; dispute contract holds stakes; DAO
  controls params).
- Where a TEE (TDX/SGX or DC-GPU CC) IS available, its attestation
  binds `snapshot_hash` into the hardware quote and counts as automatic
  quorum-satisfying evidence. Requires upgrading
  `tee-attestation-verifier` to verify real Intel/AMD quote chains
  (today it checks a configured Ed25519 signer only — a placeholder).

## 8. Failure-mode table (with mitigations)

- Blob withholding → deadline + slash, batch invalid (4.5).
- Denial-of-challenge spam → `S_ch` priced ≥ committee cost.
- Colluding operator+committee → random assignment post-anchor, public
  recompute artifacts, escalation panel.
- Systematic float drift between torch (operator) and different verifier
  stacks → Phase E: one reference implementation ratified by DAO;
  tolerance parameters published with the calibration report; dispute
  verdicts use the *reference* harness only.
- Long-response cost blow-up → max_response_len policy per gated class.

## 9. Phases

- A. This doc. ✅
- B. Reference harness + empirical drift bound on live artifacts. ✅ (Appendix A —
  `challenge_recompute.py` certified against the anchored batch-3;
  disclosure payload `jlens.activations.v1.1` adds prompt + token ids)
- C. Dispute/court contract MVP (bonds, challenge, committee, evidence, slash).
- D. Committee service as WAVS components run by validator seats.
- E. Second-stack verifier (Rust/candle) to measure inter-implementation
  drift; ratify the reference implementation via DAO.
- F. DAO parameter ratchet (k, f, W, S_op, S_ch) gated on measured stats;
  TEE quote-verification upgrade for the accelerator tier.

## 10. Open parameters for the DAO

k (committee size), f and its incentive source, W (challenge window),
S_op/S_ch per action class, max_response_len, int8 tolerance ε,
reference-implementation pinning.

---

## Appendix A — empirical drift measurement (measured, 2026-11-08)

Harness run against the *anchored* g1 batch-3 attestation
(Qwen2.5-0.5B-Instruct @ 7ae55760, 61 committed positions, CPU bf16;
same machine, same stack — the most favourable case for an exact-match
dispute design):

- per-position int8 drift: 8 positions exact, 47 drift 1 LSB,
  6 drift 2 LSB → **exact leaf/root reproduction fails even same-stack,
  same-machine**. The anchored root is reproduced exactly only from the
  *published bytes* (disclosure check), never from the verifier's own
  recompute.
- dequantized vector cosine divergence: max **9.55e-04** across all 61
  positions — provisional dispute budget 1e-3 is not comfortably above
  the measured tail; ε calibration needs cross-machine and cross-stack
  measurements before governance fixes it. This bounds the laundering
  budget: an attacker can perturb published vectors by at most ε in
  cosine without detection, so bank thresholds must operate with margin
  ≫ ε from the decision boundary (calibration report requirement).
- detection set, integrity, gate: all reproduce from recompute.
- adversarial certification: `--tamper` (fabricated leaf) → caught
  (cosine divergence 1.0 at the tampered position). `--launder`
  (replace detected positions with benign bytes) is logically exercised
  but batch-3 carries zero detections, so laundering certification
  awaits a red-gated artifact — red-team phase F deliverable.

Consequence folded into §4/§6: the dispute criterion is
**verdict- and vector-level agreement within published tolerances**, not
leaf equality. The leaf Merkle root remains the disclosure/existence
commitment (exact), and the recompute relation is the probabilistic
one.
