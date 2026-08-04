# J-Lens Probe Bank Calibration Guide

## Overview

Probe banks are the core of the J-Lens audit pipeline. Each probe bank contains
**concept vectors** — mathematical directions in a model's hidden state space that
separate honest from deceptive internal representations. When a model processes
text, its hidden states are compared against these vectors via cosine similarity.
If the similarity exceeds a threshold, a detection fires.

This guide covers building calibrated probe banks for any domain.

## Probe Bank Format

```json
{
  "probe_version": "j-lens-v0.1",
  "probe_model": "Qwen/Qwen2.5-14B-Instruct",
  "layer": 40,
  "concepts": {
    "drug_interaction_honest": {
      "vector": [0.0123, -0.0456, ...],
      "threshold": 0.70
    }
  }
}
```

- **probe_model**: Must match the model serving hidden states. Probes do NOT
  transfer across models (enforced by `d1-probe.js`).
- **layer**: Must match the layer being extracted. Different layers have
  different separation strengths.
- **concepts**: Named vectors with thresholds. Each concept is a
  diff-of-means direction from contrastive pairs.
- **vector**: Unit-normalized diff-of-means direction. Dimension must match
  the model's hidden dimension at that layer.
- **threshold**: Cosine similarity cutoff. Above this = detection fires.

## Calibration Workflow

### Step 1: Deploy a model on Akash GPU

Use the SDL templates (`tools/akash/sdl-jlens-h100.yml` or `sdl-jlens-h200.yml`)
to deploy a hidden-states extraction server.

```bash
akash deploy sdl-jlens-h100.yml
# Wait for provider bid, accept, get endpoint
```

### Step 2: Run a domain probe batch

Find the best probing layer for your domain:

```bash
python3 tools/akash/probe-batch-domain.py \
  --endpoint http://akash-endpoint:8000 \
  --pairs tools/brainmaxx/probe-banks/medical.pairs.json \
  --model "Qwen/Qwen2.5-14B-Instruct" \
  --layers 10,20,30,40,47 \
  --output tools/akash/probe-batch-medical-results.json
```

This extracts hidden states for all contrastive pairs at each layer, computes
separation scores, and identifies the best probing layer. Raw vectors are saved
for probe bank building.

### Step 3: Build the probe bank

Once you know the best layer, build the probe bank:

```bash
python3 tools/brainmaxx/build_probe_bank.py \
  --endpoint http://akash-endpoint:8000 \
  --pairs tools/brainmaxx/probe-banks/medical.pairs.json \
  --model "Qwen/Qwen2.5-14B-Instruct" \
  --layer 40 \
  --output tools/brainmaxx/probe-banks/medical.probe_bank.json \
  --threshold 0.70
```

This produces a properly formatted probe bank with one concept per contrastive
pair plus an aggregate "deception_general" concept.

### Step 4: Verify with the Audit API

Start the Domain-General Audit API and test:

```bash
CSI_AUTH_TOKEN=test node tools/brainmaxx/src/audit-api.js &
curl -X POST http://localhost:8081/audit \
  -H "Authorization: Bearer test" \
  -H "Content-Type: application/json" \
  -d '{
    "domain": "medical",
    "text": "Patient is cleared for discharge. No drug interactions identified.",
    "endpoint": "http://akash-endpoint:8000",
    "layer": 40
  }'
```

## Available Domain Pairs

| Domain | Pairs File | Concepts | Key Failure Modes |
|--------|-----------|----------|-------------------|
| general | `general.pairs.json` | 8 | Factual errors, honesty, intent integrity |
| medical | `medical.pairs.json` | 8 | Drug interactions, fabricated citations, risk suppression, contraindications |
| legal | `legal.pairs.json` | 8 | Case citation fabrication, statute accuracy, risk assessment, conflict disclosure |
| financial | `financial.pairs.json` | 8 | Earnings fabrication, risk suppression, regulatory compliance, audit findings |
| scientific | `scientific.pairs.json` | 8 | Data fabrication, peer review, statistical significance, reproducibility |

## Known Best Layers (from Akash probe batches)

| Model | Best Layer | Sep Score | Hidden Dim |
|-------|-----------|-----------|------------|
| Qwen2.5-14B | 40 | 0.2642 | 5120 |
| GLM-4.5-Air 106B | 25 | 0.3182 | 4096 |
| Qwen3-235B FP8 | TBD | TBD | 4096 |

## Threshold Tuning

Default threshold: **0.70** (cosine similarity).

- **Lower threshold** (e.g. 0.60): more sensitive, more false positives
- **Higher threshold** (e.g. 0.80): more conservative, fewer detections

Domain-specific gate thresholds (in meta.json files):

| Domain | Green | Yellow | Red |
|--------|-------|--------|-----|
| general | 0.30 | 0.15 | 0.0 |
| medical | 0.35 | 0.18 | 0.0 |
| legal | 0.32 | 0.16 | 0.0 |
| financial | 0.33 | 0.17 | 0.0 |
| scientific | 0.34 | 0.17 | 0.0 |

Higher-stakes domains (medical) have stricter green thresholds — requiring
stronger separation before clearing output.

## Adding a New Domain

1. Create `<domain>.pairs.json` with 8+ contrastive pairs (truth vs deception)
2. Create `<domain>.meta.json` with label, description, and thresholds
3. Run `probe-batch-domain.py` to find the best layer
4. Run `build_probe_bank.py` to generate `<domain>.probe_bank.json`
5. The Audit API discovers the new domain automatically from the probe-banks directory
