#!/usr/bin/env python3
"""J-Lens prover daemon (devnet primitive).

Polls a jc-mem store file for new `CommitJlens` disclosures and drives
`jlens-proof-submit.sh` for each: disclosure -> Groth16 proof ->
anchor_attestation -> resolve_with_proof -> finalized attestation.

The jc-mem store format is: 8-byte magic, then repeated
[u64 big-endian length][UTF-8 JSON Event]. This daemon tails it,
filters op.CommitJlens, and uses the event id as the attestation id
(jlens.rs: the CommitJlens event id is the attestation id convention).

Env:
  JCMEM_STORE      jc-mem store file to poll (required)
  COURT_ADDR       thought-court contract address (required)
  DEVNET_CONTAINER junod container (default junoclaw-bn254-devnet)
  SUBMIT_SCRIPT    submit primitive (default: sibling jlens-proof-submit.sh)
  WORK_DIR         scratch dir for disclosures/artifacts/ledger (required)
  POLL_SECS        poll interval (default 3)
  ONESHOT          if "1", process pending events once and exit
  TREE_HEIGHT      MiMC tree height (default 4, passed through)
"""
import json, os, struct, subprocess, sys, time

STORE = os.environ.get("JCMEM_STORE")
COURT = os.environ.get("COURT_ADDR")
WORK = os.environ.get("WORK_DIR")
# Key-aware export path: prefer `jc-mem events` (works on encrypted
# stores via --store-key) over raw file parsing.
JCMEM_BIN = os.environ.get("JCMEM_BIN", "jc-mem")
JCMEM_KEYS = os.environ.get("JCMEM_KEYS")
STORE_KEY = os.environ.get("STORE_KEY")
POLL = float(os.environ.get("POLL_SECS", "3"))
ONESHOT = os.environ.get("ONESHOT") == "1"
HERE = os.path.dirname(os.path.abspath(__file__))
SUBMIT = os.environ.get("SUBMIT_SCRIPT", os.path.join(HERE, "jlens-proof-submit.sh"))
# Git-bash, not the WSL stub: Windows resolves bare "bash" to wsl.exe.
BASH = os.environ.get("BASH", r"C:\Program Files\Gitinash.exe")
if not os.path.exists(BASH):
    BASH = "bash"
MAGIC_LEN = 8

for name, val in (("JCMEM_STORE", STORE), ("COURT_ADDR", COURT), ("WORK_DIR", WORK)):
    if not val:
        sys.exit(f"missing env {name}")
os.makedirs(WORK, exist_ok=True)
LEDGER = os.path.join(WORK, "processed.json")


def load_ledger():
    try:
        return set(json.load(open(LEDGER)))
    except Exception:
        return set()


def save_ledger(ids):
    json.dump(sorted(ids), open(LEDGER, "w"), indent=1)


def eid_hex(ev):
    """Event ids serialize as 32-byte arrays; normalize to hex."""
    raw = ev.get("id")
    if isinstance(raw, list):
        return bytes(raw).hex()
    return raw


def read_events_cli():
    """Ask jc-mem for CommitJlens events — key-aware, works on
    JCMEMe0 encrypted stores when STORE_KEY/JCMEM_KEYS are set."""
    if not JCMEM_KEYS:
        return None
    cmd = [JCMEM_BIN, "events", "--keys", JCMEM_KEYS, "--store", STORE,
           "--op", "CommitJlens"]
    if STORE_KEY:
        cmd += ["--store-key", STORE_KEY]
    r = subprocess.run(cmd, capture_output=True, text=True, timeout=120)
    if r.returncode != 0:
        return None
    return [json.loads(line) for line in r.stdout.splitlines() if line.strip()]


def read_events():
    """Events via the jc-mem CLI when keys are configured; raw store
    parse otherwise (plaintext stores only)."""
    cli = read_events_cli()
    if cli is not None:
        return cli
    out = []
    if not os.path.exists(STORE):
        return out
    with open(STORE, "rb") as f:
        magic = f.read(MAGIC_LEN)
        if len(magic) < MAGIC_LEN:
            return out
        while True:
            len_buf = f.read(8)
            if len(len_buf) < 8:
                break
            (ln,) = struct.unpack(">Q", len_buf)
            if ln == 0 or ln > 4 * 1024 * 1024:
                break
            buf = f.read(ln)
            if len(buf) < ln:
                break  # torn write at tail; retry next poll
            try:
                ev = json.loads(buf)
            except Exception:
                break
            out.append(ev)
    return out


def disclosure_of(ev):
    op = ev.get("op") or {}
    r = (op.get("CommitJlens") or {}).get("readout")
    if not r:
        return None
    return {
        "activations": r["activations"],
        "bank_vec": r["bank_vec"],
        "threshold": r["threshold"],
        "leaf_index": r["leaf_index"],
        **({"mimc_path": r["mimc_path"]} if r.get("mimc_path") else {}),
    }


def process(ev):
    eid = eid_hex(ev)
    disc = disclosure_of(ev)
    if disc is None:
        return "skip"
    dpath = os.path.join(WORK, f"disclosure-{eid[:16]}.json")
    json.dump(disc, open(dpath, "w"))
    env = dict(os.environ, ATTESTATION_ID=eid)
    r = subprocess.run(
        [BASH, SUBMIT, dpath],
        env=env, capture_output=True, text=True, timeout=600,
    )
    sys.stdout.write(r.stdout)
    if r.returncode != 0:
        sys.stderr.write(r.stderr[-2000:])
        return "fail"
    return "ok"


def main():
    seen = load_ledger()
    print(f"jlens-proverd: polling {STORE} (court {COURT[:16]}...)", flush=True)
    while True:
        for ev in read_events():
            eid = eid_hex(ev)
            if not eid or eid in seen:
                continue
            res = process(ev)
            print(f"  event {eid[:16]}... -> {res}", flush=True)
            if res in ("ok", "skip"):
                seen.add(eid)
                save_ledger(seen)
            # "fail" events retry on next poll — a bad disclosure should
            # not block the stream forever, but surface loudly.
        if ONESHOT:
            return
        time.sleep(POLL)


if __name__ == "__main__":
    main()
