# J-Lens — D1 audit probe for Brainmaxx

Implements PLAN_J_REEF_AND_J_LENS.md §3 / A18c-9 Phase 3: a linear-readout
probe over a model's hidden states, run *before* a D2 draft is finalized,
looking for forbidden concepts (reward hacking, ignore-instructions,
deception, ...). Local, operator-side, feature-flagged, off by default.

This directory holds the **Python side** (model forward pass, tensor math —
things Node cannot do without a heavy new dependency, which v0's "zero new
npm dependency" rule forbids). The **Node side** (`../src/d1-probe.js`) stays
pure JS: it only reads the two JSON files these scripts produce and does a
deterministic dot-product/threshold check. Nothing non-deterministic ever
touches the Brainmaxx trace file directly — only the finished, hashed
`j_space_snapshot`.

```
build_probe_bank.py   (once per model+concept-set)  -> probe_bank.json
extract_hidden_states.py (once per draft to audit)  -> hidden_states.json
                                          |
                                          v
brainmaxx j-lens <run_id> --hidden-states hidden_states.json --probe-bank probe_bank.json
```

## Quickstart — small open-weight model (works on a laptop CPU)

```bash
cd tools/brainmaxx/j-lens
pip install -r requirements.txt

# 1. Build the probe bank once (diff-of-means over contrastive examples)
python build_probe_bank.py \
  --model Qwen/Qwen2.5-0.5B-Instruct \
  --examples examples/concepts.json \
  --layer 12 \
  --out probe_bank.json

# 2. Extract hidden states for a specific draft/text you want to audit
python extract_hidden_states.py \
  --model Qwen/Qwen2.5-0.5B-Instruct \
  --text "$(cat /path/to/draft.md)" \
  --layer 12 \
  --out hidden_states.json

# 3. Attach the snapshot to a Brainmaxx trace
cd ../..
node tools/brainmaxx/src/cli.js j-lens <run_id> \
  --hidden-states tools/brainmaxx/j-lens/hidden_states.json \
  --probe-bank tools/brainmaxx/j-lens/probe_bank.json
```

`Qwen2.5-0.5B-Instruct` is the recommended first target: Apache-2.0,
~1GB in fp32, runs on CPU in seconds, and is one of the models shortlisted
in PLAN_J_REEF_AND_J_LENS.md §5 Phase 3. Use this to prove the pipeline
end-to-end and reproduce a baseline detection rate before touching anything
bigger.

## Kimi K3 integration path

Kimi K3 (Moonshot AI, open weights released 2026-07-27) is a 2.8T-parameter
MoE model. This changes what "run the model" means, but **not** the JSON
contract above — `d1-probe.js` and the trace-attachment flow are unchanged.
Only `build_probe_bank.py` / `extract_hidden_states.py` need a different
backend.

### Why the hosted API path does not work for J-Lens

Every hosted Kimi K3 endpoint found so far (Moonshot's own API, third-party
inference resellers) is OpenAI-compatible: it returns tokens and optionally
logprobs, never `hidden_states`. A linear readout on the residual stream
needs the actual activations mid-forward-pass. **A hosted completions API
cannot produce a genuine J-Lens attestation, no matter how good the API
is** — this is the same constraint the DAO already wrote down in
A18c-9 §"Why open-weight models are required." Closed inference = no J-Lens.

### What does work: self-hosted inference with an activation hook

1. **Quantize the weights.** Community GGUF/AWQ quantizations of Kimi K3
   (Q4-class) run to roughly 300-700GB depending on quant level — still far
   past a single consumer GPU, but within reach of a rented multi-GPU node.
2. **Serve with an inference engine that exposes hidden states.** `vLLM`
   supports `output_hidden_states`-style hooks via its Python API (not the
   OpenAI-compatible HTTP server — that path strips activations the same
   way hosted APIs do). Alternative: `llama.cpp`'s C API exposes intermediate
   tensor buffers via `ggml` callbacks, usable from a small Python/ctypes
   shim if vLLM's MoE support lags for this model.
3. **Rent the compute.** This is where the DAO's Akash outreach work
   (A035, passed and executed) is directly useful: an Akash GPU provider
   with enough VRAM (8x H100/H200 class, or equivalent multi-node) can host
   a quantized Kimi K3 instance for the duration of a probe-bank build +
   a batch of `extract_hidden_states` runs, then be torn down.

   **Update 2026-07-28: Akash Confidential Compute is now live.** AEP-83
   ("Confidential Compute via Kata Containers") is Final (completed
   2026-07-17), and provider software v0.16.0-rc0 ships per-pod TEE
   attestation for both CPU (AMD SEV-SNP / Intel TDX) and GPU (`cpu-gpu`
   TEE type, VFIO passthrough, CC-on mode) workloads, with an attestation
   sidecar injected by default. This is the network-level capability A035
   was chasing. It does NOT automatically mean a suitable provider is
   live and audited right now — check `tee/type` in
   `console-api.akash.network/v1/providers` (or the equivalent SDL
   placement match) for a `cpu-gpu`-capable provider with enough VRAM for
   quantized Kimi K3 before assuming Phase D is unblocked. If one exists,
   Phase D collapses into Phase B/C — attestation can be requested on the
   same rented deployment instead of waiting on a separate future step.
4. **Point the scripts at the rented endpoint.** `build_probe_bank.py` and
   `extract_hidden_states.py`'s `--model` argument becomes a local path on
   the rented node (or a thin wrapper class swapped in for
   `AutoModelForCausalLM` if vLLM's Python API is used instead of
   `transformers`) — the diff-of-means math, the JSON schema, and
   `d1-probe.js` do not change.
5. **Pick a layer.** Kimi K3's exact layer count/hidden size is not yet
   reflected in this repo (no local copy of the config has been fetched).
   The build script should be run once with `--layer` swept over a few
   candidate mid-network layers (typically 40-60% of total depth is a
   reasonable starting search range per the tuned-lens literature) and the
   probe bank with the cleanest positive/negative separation (largest
   diff-of-means norm relative to within-class variance) kept.

### Phased rollout for this pilot

- **Phase A (done today, this repo)**: `d1-probe.js` + CLI wiring +
  deterministic tests, proven against `Qwen2.5-0.5B-Instruct` on CPU.
- **Phase B**: rent a GPU node (Akash or otherwise), quantize/serve Kimi K3,
  sweep `--layer`, build the first real probe bank against
  `examples/concepts.json` (or an expanded set).
- **Phase C**: run `extract_hidden_states.py` against a real DAO draft
  (e.g. a Brainmaxx `plan` output) on the rented Kimi K3 instance, attach
  the snapshot via `brainmaxx j-lens`, and post the resulting trace-export
  to Moultbook as the first Kimi K3 J-Lens result.
- **Phase D**: TEE attestation of the forward pass. Previously blocked on
  provider capability; as of 2026-07-28, Akash Confidential Compute
  (AEP-83) is live at the protocol level. Check for an actual
  `tee/type: cpu-gpu` provider with sufficient VRAM before treating this
  as unblocked — if one exists, this folds into Phase B/C rather than
  requiring a separate future proposal.

No DAO funds are required for Phase A. Phase B requires GPU rental cost —
see the A039 proposal draft for the funding/mandate boundary.

### A040 provider audit (2026-07-29 snapshot)

Queried `console-api.akash.network/v1/providers` per A040's mandate.
Finding: **no provider in the sampled response advertises a
`tee/type: cpu-gpu` or SEV-SNP/TDX confidential-compute attribute key.**
AEP-83 is live at the protocol level (provider software v0.16.0-rc0
supports it), but individual provider adoption/attribute-tagging has not
caught up yet as of this date. Phase D remains blocked pending provider
rollout — re-check per A040's "document the gap and re-check
periodically" directive.

GPU capacity observed for Phase B (non-TEE), useful for sizing a
quantized pilot run — largest single cards seen in this pass:

| Provider | GPU | VRAM |
|---|---|---|
| skyfall.cz (Virginia) | NVIDIA L40S | 48GiB |
| globalrad.cloud | NVIDIA RTX 5090 | 32GiB |
| sow-provider.xyz (Tokyo) | NVIDIA RTX 5090 | 32GiB |
| pandoraa50.com | NVIDIA RTX 3090 | 24GiB |
| onenode.dev (Sydney) | NVIDIA RTX 4080 | 16Gi |

No 8x H100/H200-class multi-GPU node was visible in this pass — the
registry has thousands of providers and this was a partial sample, not
an exhaustive scan. **No multi-hundred-GB VRAM node has been confirmed
yet** for a non-quantized or lightly-quantized Kimi K3 run; a Q4-class
quantization (roughly 300-700GB total) still exceeds what any single
card above provides, and would require either a multi-GPU aggregate
lease or a much more aggressive quantization (sub-4-bit / selective
layer offload) to fit an individual 32-48GiB card. Re-run this query
with pagination/filtering before committing to a specific provider.
