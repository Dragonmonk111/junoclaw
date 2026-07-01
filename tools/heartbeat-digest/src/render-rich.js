/**
 * Rich heartbeat digest renderers.
 *
 * Outputs DAO DAO-ready markdown and the original plain markdown
 * from the same structured digest data.
 */

function pad2(n) {
  return String(n).padStart(2, '0')
}

function formatDateTime(iso) {
  const d = new Date(iso)
  return `${d.getUTCFullYear()}-${pad2(d.getUTCMonth() + 1)}-${pad2(d.getUTCDate())} ${pad2(
    d.getUTCHours(),
  )}:${pad2(d.getUTCMinutes())}:${pad2(d.getUTCSeconds())} UTC`
}

function statusLabel(status) {
  const map = {
    open: 'Open',
    passed: 'Passed (ready to execute)',
    executed: 'Executed',
    rejected: 'Rejected',
    closed: 'Closed',
    execution_failed: 'Execution failed',
  }
  return map[status] || status
}

function badge(status) {
  const map = {
    open: '🔵 Open',
    passed: '🟢 Passed',
    executed: '🟣 Executed',
    rejected: '🔴 Rejected',
    closed: '⚪ Closed',
    execution_failed: '⚠️ Execution failed',
  }
  return map[status] || status
}

function voteBar({ yes = 0, no = 0, abstain = 0, total = 0 } = {}) {
  const totalVotes = yes + no + abstain
  if (!totalVotes) return 'No votes yet'
  const y = ((yes / totalVotes) * 100).toFixed(1)
  const n = ((no / totalVotes) * 100).toFixed(1)
  const a = ((abstain / totalVotes) * 100).toFixed(1)
  return `Yes ${y}% | No ${n}% | Abstain ${a}%`
}

function renderPlain({ date, proposals, members, treasury, meta }) {
  const newToday = proposals.filter((p) => p.is_new_today)
  const needsVotes = proposals.filter((p) => p.status === 'open')
  const readyToExecute = proposals.filter((p) => p.status === 'passed')
  const closingSoon = proposals.filter((p) => p.status === 'open' && p.is_closing_soon)
  const closed = proposals.filter((p) =>
    ['executed', 'rejected', 'closed', 'execution_failed'].includes(p.status),
  )

  const treasuryLines = treasury.length
    ? treasury.map((b) => `- ${b.amount} ${b.denom}`).join('\n')
    : '- 0 ujuno (treasury empty)'

  const totalPower = members.reduce((sum, m) => sum + Number(m.weight || 0), 0)
  const memberLines = members.length
    ? members.map((m) => `- ${m.addr} — weight ${m.weight}`).join('\n')
    : '- Members query unavailable'

  return `# Juno Agents DAO Heartbeat Digest — ${date}

_Generated from on-chain data via public RPC._

## Quick stats
| Metric | Value |
|---|---|
| DAO core | ${meta.dao_core} |
| Total proposals | ${proposals.length} |
| Open | ${needsVotes.length} |
| Passed / ready to execute | ${readyToExecute.length} |
| Closed | ${closed.length} |
| Total voting power | ${totalPower} |

## New today
${newToday.length
    ? newToday.map((p) => `- **A${p.id}**: ${p.title || '(no title)'} — ${statusLabel(p.status)}`).join('\n')
    : '- none'}

## Needs votes
${needsVotes.length
    ? needsVotes.map((p) => `- **A${p.id}**: ${p.title || '(no title)'} [vote](https://dao.daodao.zone/dao/${meta.dao_core}/proposals/${p.id})`).join('\n')
    : '- none'}

## Ready to execute
${readyToExecute.length
    ? readyToExecute.map((p) => `- **A${p.id}**: ${p.title || '(no title)'}`).join('\n')
    : '- none'}

## Closing soon (next 24h)
${closingSoon.length
    ? closingSoon.map((p) => `- **A${p.id}**: ${p.title || '(no title)'}`).join('\n')
    : '- none'}

## Closed since last digest
${closed.length
    ? closed.map((p) => `- **A${p.id}**: ${p.title || '(no title)'} — ${statusLabel(p.status)}`).join('\n')
    : '- none'}

## Treasury
${treasuryLines}

## Members
${memberLines}

## Data sources
- DAO core: ${meta.dao_core}
- Proposal module: ${meta.proposal_module}
- REST endpoint: ${meta.rest_endpoint}
- Generated at: ${formatDateTime(meta.generated_at)}
`
}

function renderRichMarkdown({ date, summary, proposals, members, treasury, meta }) {
  const sections = {
    new_today: proposals.filter((p) => p.is_new_today),
    needs_votes: proposals.filter((p) => p.status === 'open'),
    ready_to_execute: proposals.filter((p) => p.status === 'passed'),
    closing_soon: proposals.filter((p) => p.status === 'open' && p.is_closing_soon),
    closed: proposals.filter((p) =>
      ['executed', 'rejected', 'closed', 'execution_failed'].includes(p.status),
    ),
  }

  const treasuryLines = treasury.length
    ? treasury
        .map(
          (b) =>
            `- **${b.amount}** ${b.denom} ${
              b.denom === 'ujuno' ? `(≈ ${(Number(b.amount) / 1e6).toFixed(2)} JUNO)` : ''
            }`,
        )
        .join('\n')
    : '- 0 ujuno (treasury empty)'

  const memberLines = members.length
    ? members
        .map(
          (m) =>
            `- **${m.addr}** — weight ${m.weight} ${m.role ? `(${m.role})` : ''}`,
        )
        .join('\n')
    : '- Members query unavailable'

  const proposalRows = (list) =>
    list.length
      ? list
          .map(
            (p) =>
              `| **A${p.id}** | ${badge(p.status)} | ${p.title || '(no title)'} | ${voteBar(
                p.votes,
              )} | ${
                p.status === 'open'
                  ? `[Vote ↗](https://dao.daodao.zone/dao/${meta.dao_core}/proposals/${p.id})`
                  : `[View ↗](https://dao.daodao.zone/dao/${meta.dao_core}/proposals/${p.id})`
              } |`,
          )
          .join('\n')
      : '| — | — | No items in this section | — | — |'

  const sectionHeader = (icon, title, count) =>
    `### ${icon} ${title} ${count ? `(${count})` : ''}`

  const watcherLines = [
    meta.block_height ? `- Observed at block: \`${meta.block_height}\`` : null,
    meta.trigger_reason ? `- Trigger: \`${meta.trigger_reason}\`` : null,
    meta.previous_moultbook ? `- Cites previous heartbeat: \`${meta.previous_moultbook}\`` : null,
  ].filter(Boolean)

  return `# 🐚 Juno Agents DAO Heartbeat — ${date}

> The DAO's daily pulse, anchored on-chain. This digest is a living shell: a public record of proposals, votes, treasury, and membership so every agent and voter can see the reef's state at a glance.

<p align="center">
  <img src="https://raw.githubusercontent.com/Dragonmonk111/junoclaw/main/junoclaw/frontend/public/mascot.png" width="120" alt="JunoClaw mascot" />
</p>

---

## 📊 Snapshot

| Open | Passed | Ready to execute | Closed | Total proposals | Voting power | Treasury |
|---|---|---|---|---|---|---|
| **${summary.open}** | **${summary.passed}** | **${summary.ready_to_execute}** | **${summary.closed}** | **${summary.total_proposals}** | **${summary.total_voting_power}** | ${treasury.length ? `${(Number(treasury[0].amount) / 1e6).toFixed(2)} JUNO` : '0 JUNO'} |

${sectionHeader('🆕', 'New today', sections.new_today.length)}

| ID | Status | Title | Votes | Link |
|---|---|---|---|---|
${proposalRows(sections.new_today)}

${sectionHeader('🗳️', 'Needs votes', sections.needs_votes.length)}

| ID | Status | Title | Votes | Link |
|---|---|---|---|---|
${proposalRows(sections.needs_votes)}

${sectionHeader('✅', 'Ready to execute', sections.ready_to_execute.length)}

| ID | Status | Title | Votes | Link |
|---|---|---|---|---|
${proposalRows(sections.ready_to_execute)}

${sectionHeader('⏰', 'Closing soon (next 24h)', sections.closing_soon.length)}

| ID | Status | Title | Votes | Link |
|---|---|---|---|---|
${proposalRows(sections.closing_soon)}

${sectionHeader('📁', 'Closed since last digest', sections.closed.length)}

| ID | Status | Title | Votes | Link |
|---|---|---|---|---|
${proposalRows(sections.closed)}

---

## 🏛️ Members

${memberLines}

**Total voting power:** ${summary.total_voting_power}

---

## 💰 Treasury

${treasuryLines}

---

## 🔗 Citation & data sources

- DAO core: \`${meta.dao_core}\`
- Proposal module: \`${meta.proposal_module}\`
- Moultbook entry: \`${meta.moultbook || 'pending'}\`
${watcherLines.length ? watcherLines.join('\n') + '\n' : ''}- REST endpoint: \`${meta.rest_endpoint}\`
- Generated at: ${formatDateTime(meta.generated_at)}
- This digest is published as a public **Moultbook** entry by the DAO's agent.

---

_The reef is small today. The shells are few. But every moult adds sediment. Every proposal cites the last. The ocean remembers._
`
}

export function renderDigest(data) {
  return {
    plain: renderPlain(data),
    rich: renderRichMarkdown(data),
  }
}
