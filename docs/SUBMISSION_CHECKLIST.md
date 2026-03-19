# Proposal Submission Checklist — Tonight (March 18, 2026)

## Context
- Jake endorsed on Telegram (Junø 🤝 AI, 4578 members, "meow" + "We are so back")
- Jake amended the HackMD file (co-editing = strong signal)
- Discord traction from Rattadan (nride/netadao)
- 4 fire reacts on Telegram pinned message
- HackMD link: https://hackmd.io/s/HyZu6qv5Zl

---

## Pre-Flight (Do These First)

- [ ] **Check Jake's HackMD amendments**: Open https://hackmd.io/s/HyZu6qv5Zl and compare to `GOV_PROP_COPYPASTE.md`. If Jake changed wording, update the copy-paste version to match. His edits carry weight.
- [ ] **Verify deposit amount**: Go to https://ping.pub/juno/gov → look at recent proposals. Current minimum is likely **250 JUNO**. Confirm your wallet has enough.
- [ ] **Confirm wallet**: `juno1tvpe72amnd3arnh4nhlf3hztx5aqznu6hz5f4m` — ensure Keplr has it loaded and you're on **juno-1** (mainnet, NOT uni-7).
- [ ] **Gas**: Ensure you have extra JUNO beyond deposit for gas (~0.025 JUNO).
- [ ] **GitHub push**: Make sure the latest code is pushed to https://github.com/Dragonmonk111/junoclaw so the links in the proposal are valid.

---

## Submission Steps

### Option A: ping.pub (Preferred)

1. Go to https://ping.pub/juno/gov
2. Click **New Proposal** → **Text Proposal** (signaling)
3. **Title**: Copy from `GOV_PROP_COPYPASTE.md` FIELD 1
4. **Description**: Copy from `GOV_PROP_COPYPASTE.md` FIELD 2 (or the live HackMD if Jake's version is better)
5. **Deposit**: Enter the minimum (likely 250 JUNO)
6. **Submit** → Keplr will pop up → Approve
7. **Record**: Note the proposal number and TX hash

### Option B: CLI (If ping.pub is down)

```bash
junod tx gov submit-proposal \
  --title "JunoClaw — Verifiable AI Agents with TEE-Attested Junoswap Revival on Juno" \
  --description "$(cat GOV_PROP_DESCRIPTION.txt)" \
  --type Text \
  --deposit 250000000ujuno \
  --from vairagyanodes \
  --chain-id juno-1 \
  --node https://juno-rpc.polkachu.com:443 \
  --gas auto \
  --gas-adjustment 1.3 \
  --gas-prices 0.025ujuno \
  --yes
```

> Note: For CLI, you'd need to save FIELD 2 to a plain text file first. Escape any special characters.

### Option C: Alternative Frontends

- https://station.juno.network/gov
- https://app.juno.network
- https://daodao.zone

---

## Immediately After Submission

- [ ] **Copy the proposal number** (e.g., Proposal #47)
- [ ] **Copy the TX hash** from Keplr confirmation
- [ ] **Announce on Telegram**: Reply to the pinned HackMD message with: "Proposal #{N} is live on juno-1. Vote here: https://ping.pub/juno/gov/{N}"
- [ ] **Announce on Discord**: Post in #general with the same link
- [ ] **Update HackMD**: Add "STATUS: Live on juno-1 as Proposal #{N}" at the top
- [ ] **Update GOV_PROP_COPYPASTE.md**: Record proposal number and TX hash
- [ ] **Tweet** (optional): "JunoClaw signaling proposal live on @JunoNetwork. Verifiable AI agents + Junoswap revival. Proposal #{N}: [link]"

---

## Timing Strategy

Jake is active NOW (19:41 his time based on Telegram screenshot). Submit while he's online so he can:
1. See it immediately
2. Potentially vote or comment
3. Share with WAVS/Layer team

Best window: **within the next 2-3 hours**.

---

## Risk Check

- Deposit is returnable if proposal passes OR if it's vetoed by less than 33.4%
- Signaling proposals are low-risk — no code execution
- If NoWithVeto > 33.4%, deposit is burned. Given Jake's endorsement, this risk is minimal.
