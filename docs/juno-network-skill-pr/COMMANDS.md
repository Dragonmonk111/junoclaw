# Copy-paste command sequence — Open the `juno-network-skill` PR

*Click-paste path to land `references/junoclaw.md` as a PR on `CosmosContracts/juno-network-skill`. ~10 minutes user-side.*

## Pre-requisite

- GitHub CLI (`gh`) installed and authenticated as `Dragonmonk111`, OR
- A web browser logged into GitHub as `Dragonmonk111`

If `gh` is not installed: `winget install GitHub.cli`, then `gh auth login`.

## Option A — `gh` CLI (recommended, ~5 min)

```powershell
# 1. Fork the upstream repo to Dragonmonk111
gh repo fork CosmosContracts/juno-network-skill --clone --remote

# 2. cd into the cloned fork (gh creates ./juno-network-skill)
#    NOTE: run this from your usual workspace root, e.g. C:\Users\Taj\
#    Adjust if you already have a juno-network-skill clone elsewhere.

# 3. Create the feature branch
git -C juno-network-skill checkout -b feat/junoclaw-reference

# 4. Copy the staged file into place
Copy-Item `
  -Path "c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\docs\juno-network-skill-pr\references\junoclaw.md" `
  -Destination "juno-network-skill\references\junoclaw.md"

# 5. Commit (signed if possible — GPG / Sigstore / SSH key, doesn't matter which)
git -C juno-network-skill add references/junoclaw.md
git -C juno-network-skill commit -m "docs(references): add references/junoclaw.md — JunoClaw agent-company skill reference"

# 6. Push the branch
git -C juno-network-skill push -u origin feat/junoclaw-reference

# 7. Open the PR. The PR body is in
#    c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\docs\juno-network-skill-pr\PR_BODY.md
#    — copy from "## Title" through end of "## Body". Then run:
gh pr create `
  --repo CosmosContracts/juno-network-skill `
  --base main `
  --head Dragonmonk111:feat/junoclaw-reference `
  --title "docs(references): add references/junoclaw.md — JunoClaw agent-company skill reference" `
  --body-file "c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\docs\juno-network-skill-pr\PR_BODY.md"
```

`gh pr create` will print the PR URL. Open it in a browser, double-check the rendering, and that's it.

## Option B — Web UI (~10 min)

1. **Fork.** Go to https://github.com/CosmosContracts/juno-network-skill → click **Fork** → keep default name.
2. **Clone the fork locally** (only if you want to keep a local copy; otherwise edit in browser):
   ```powershell
   git clone https://github.com/Dragonmonk111/juno-network-skill.git
   ```
3. **In browser:** on your fork's page, click **Add file** → **Create new file** → set path to `references/junoclaw.md`.
4. **Paste** the contents of `c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\docs\juno-network-skill-pr\references\junoclaw.md` into the editor.
5. **Commit message:** `docs(references): add references/junoclaw.md — JunoClaw agent-company skill reference`
6. **Commit directly to a new branch:** `feat/junoclaw-reference`. Click **Propose new file**.
7. On the resulting PR-creation page, set base branch = `main`, head branch = `Dragonmonk111:feat/junoclaw-reference`.
8. **Title:** `docs(references): add references/junoclaw.md — JunoClaw agent-company skill reference`
9. **Body:** paste from `c:\cosmos-node\node-data\config\CascadeProjects\windsurf-project\junoclaw\docs\juno-network-skill-pr\PR_BODY.md` (the section starting with "Adds a new reference file...").
10. Click **Create pull request**.

## Post-PR (immediately)

- Add a one-line message in the Telegram thread once the PR URL is live:

  > Hey — PR up at `<URL>`. Mirrors the format of `references/dao-dao.md`; mainnet code IDs marked TBD pending JunoClaw's mainnet deploy. Happy to revise to fit the skill's tone better.

  This is the *only* legitimate next DM. No status updates, no other asks. The PR is the deliverable.

- Update [`memory/lessons-2026-05-17.md`](../../memory/lessons-2026-05-17.md) §4 with the actual PR URL once it's live.

## Rollback

If you change your mind before the PR is opened, just delete the branch:

```powershell
git -C juno-network-skill push origin --delete feat/junoclaw-reference
git -C juno-network-skill branch -D feat/junoclaw-reference
```

If the PR is already opened and you want to withdraw:

```powershell
gh pr close --repo CosmosContracts/juno-network-skill <PR-number> --comment "Closing — will re-open with revisions per maintainer feedback."
```

Apache-2.0. Drafted 2026-05-17.
