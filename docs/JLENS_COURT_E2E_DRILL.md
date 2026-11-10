# J-Lens thought-court — dispute-path e2e drill (junoclaw-g1)

The court's happy path (anchor → challenge window → finalize) is proven.
This drill exercises the **untested half**: sortition → verdicts → quorum
resolution → slashing, plus the timeout/DAO-fallback path.

Court: `juno10c3dunkwh68fgufsd4cw8q24e0wjcsh9290c0xvy22hfaqjprxds63sy32`
(code 24). committee_size=3, quorum_bp=6700 → early resolution needs
**all 3** seats voting the same way; full-committee simple majority
otherwise; exact tie stays open to verdict_deadline.

## Prerequisites

```bash
export JUNOCLAW_WALLET_PASSPHRASE=...   # or keychain backend
# JUNOCLAW_GRPC_ENDPOINT if non-default
cd mcp
```

Wallet map (all enrolled, juno-prefixed):

| role | wallet id | address |
|---|---|---|
| operator + challenger + admin | `jlens-ops` | juno1t08k74tqwukkxjyq5cwqrguzs7ktv4y7jfr4d6 |
| verifier seat 1 | `builder` | juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z |
| verifier seat 2 | `dao-truth-operator` | juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd |
| verifier seat 3 | `dao-verdict-helper` | juno1jpgd0vcveam90smx5rk6cuycv9lfemcpasf0fv |
| verifier seat 4 (spare) | `relayer-soak` | juno1uzwj8lk4nqap5v5suv3a20zhmyghp2j3ac0tsh |

## Phase A — fund + register seats

```bash
COURT=juno10c3dunkwh68fgufsd4cw8q24e0wjcsh9290c0xvy22hfaqjprxds63sy32

# 2.2M each: 2M verifier bond + gas buffer
for w in juno1aq995jf4fezcghl6ar6k79hk9layss8w6q2t7z \
         juno16kmhmkyf6n4hnue0l7dkcuexajxh44lgv75utd \
         juno1jpgd0vcveam90smx5rk6cuycv9lfemcpasf0fv; do
  npx tsx src/jlens-court.ts --wallet jlens-ops --send $w --amount 2200000
done

# register (each with own wallet)
npx tsx src/jlens-court.ts --contract $COURT --wallet builder             --register-verifier --bond 2000000
npx tsx src/jlens-court.ts --contract $COURT --wallet dao-truth-operator  --register-verifier --bond 2000000
npx tsx src/jlens-court.ts --contract $COURT --wallet dao-verdict-helper  --register-verifier --bond 2000000

npx tsx src/jlens-court.ts --contract $COURT --query '{"list_verifiers":{"limit":10}}'
```

## Phase B — clean dispute, quorum resolution

```bash
# fresh attestation (synthetic root ok for the drill; a real batch root better)
ROOT=$(python -c "import secrets;print(secrets.token_hex(32))")
npx tsx src/jlens-court.ts --contract $COURT --wallet jlens-ops --anchor $ROOT --activation-root $ROOT --bond 1000000

# challenge (same wallet challenging its own attestation is legal — contract
# does not prohibit; economics are what the dispute is FOR)
npx tsx src/jlens-court.ts --contract $COURT --wallet jlens-ops --challenge $ROOT --bond 500000
npx tsx src/jlens-court.ts --contract $COURT --query-dispute $ROOT
# expect: committee = all 3 seats (k=3 of 3), votes=[], deadline = now+43200s
```

**Committee-service path (the real seat behavior):**

```bash
cd ../tools/brainmaxx/jlens-serve
python committee_service.py --court $COURT --mcp-dir ../../../mcp \
  --wallet builder --artifacts ./disclosures \
  --bank ../j-lens/probe_bank_onchain_v7_calibrated.json \
  --model Qwen/Qwen2.5-0.5B-Instruct --once
# expect: seat detected -> recompute -> "skip:no-disclosure-bundle"
# (proves poll->seat-detection logic; the harness needs the operator's
# disclosure bundle — pull jlens-attestations-g1-v1 from the release if
# you want a REAL verdict through the service)
```

**Manual verdict path (what actually resolves the drill):**

```bash
EV=$(python -c "import hashlib;print(hashlib.sha256(b'drill-evidence').hexdigest())")
npx tsx src/jlens-court.ts --contract $COURT --wallet builder             --verdict $ROOT --verdict-kind clean --evidence-hash $EV
npx tsx src/jlens-court.ts --contract $COURT --wallet dao-truth-operator  --verdict $ROOT --verdict-kind clean --evidence-hash $EV
npx tsx src/jlens-court.ts --contract $COURT --wallet dao-verdict-helper  --verdict $ROOT --verdict-kind clean --evidence-hash $EV
```

**Expected on 3rd vote (early quorum):** attestation → `finalized`
(Clean verdict); operator gets bond + 50% of challenger bond; admin gets
the rest; committee seat counts drop back to 0. Verify:

```bash
npx tsx src/jlens-court.ts --contract $COURT --query-attestation $ROOT
```

Also exercise the fraud path by running Phase B again with
`--verdict-kind fraud` — expect `fraudulent`, operator bond split
50% challenger / 50% admin.

## Phase C — stalled committee → timeout → DAO force-resolve

```bash
ROOT2=$(python -c "import secrets;print(secrets.token_hex(32))")
npx tsx src/jlens-court.ts --contract $COURT --wallet jlens-ops --anchor $ROOT2 --activation-root $ROOT2 --bond 1000000
npx tsx src/jlens-court.ts --contract $COURT --wallet jlens-ops --challenge $ROOT2 --bond 500000

# ONE vote only, then wait out verdict_deadline (43200s — OR temporarily
# UpdateConfig verdict_window_secs to ~120 for the drill, restore after)
npx tsx src/jlens-court.ts --contract $COURT --wallet builder --verdict $ROOT2 --verdict-kind clean --evidence-hash $EV

npx tsx src/jlens-court.ts --contract $COURT --wallet jlens-ops --update-config --verdict-window-secs 120
# ...wait ~2min...
npx tsx src/jlens-court.ts --contract $COURT --resolve-timeout $ROOT2   # anyone
# expect: status unresolved, challenger bond refunded, seats freed
npx tsx src/jlens-court.ts --contract $COURT --wallet jlens-ops --force-resolve $ROOT2 --verdict-kind fraud
# expect: status fraudulent; slash_ministry payouts per fraud path
npx tsx src/jlens-court.ts --contract $COURT --wallet jlens-ops --update-config --verdict-window-secs 43200
```

## Phase D — negative checks + teardown

- double vote → `AlreadyVoted`; non-committee vote → `NotCommittee`;
  post-deadline challenge → `ChallengeWindowClosed`; remove_verifier
  while seated → `VerifierBusy`.
- After all disputes resolve: `--remove-verifier` ×3 (bonds return),
  then `--wallet jlens-ops --send` any remaining seat balance back if
  desired.
- Record drill txs in `deploy/deployed-junoclaw-g1.json` under the
  thought-court entry.

## Honest scope notes

- `committee_service.py` runs the REAL seat protocol (disclosure-bundle
  recompute → evidence_hash → signed verdict). The manual `--verdict`
  path exercises the contract mechanics; the service path exercises the
  verifier behavior. Both needed before calling the court proven.
- Committee_size=3 with 3 registered seats means sortition always seats
  everyone — Fiat-Shamir selection is real but untested for k<n until a
  4th+ seat registers (spare: relayer-soak).
