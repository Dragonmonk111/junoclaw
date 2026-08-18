// ── Robot Ops fleet simulation ──
//
// Cortex Console's "Robot Ops" tab visualizes the two-tier trust
// architecture (reflex tier vs intent tier — see ARTICLE_TRUST_OPERATING_SYSTEM):
// per-joint J-Lens verdicts, a Truth Market operator constellation, and a
// BFT-batch flight recorder. None of `truth-market` / `task-ledger` /
// `marketplace` are deployed to uni-7 yet, so this module drives the panel
// with a local, self-contained simulation. The data shapes mirror the real
// contracts' query responses (`consensus_verdict: "green"|"red"`,
// `batch_height`, operator addresses) so swapping this hook for real
// `CosmWasmClient.queryContractSmart` calls later is a drop-in change —
// only the producer of `FleetSimState` needs to change, not the panel.

import { useEffect, useRef, useState } from 'react'

export type Verdict = 'green' | 'amber' | 'red'

export interface JointDef {
  id: string
  label: string
  row: number
  col: number
}

export interface JointNode extends JointDef {
  verdict: Verdict
  confidence: number
}

export type Morphology = 'biped' | 'quadruped' | 'arm'

export interface RobotState {
  id: string
  name: string
  morphology: Morphology
  cols: number
  rows: number
  joints: JointNode[]
  confidenceHistory: number[]
  health: number
}

export interface OperatorNode {
  id: string
  label: string
  agrees: boolean
}

export type ActionStatus = 'pending' | 'probing' | 'approved' | 'blocked'

export interface ActionVote {
  operatorId: string
  agrees: boolean
}

export interface ActionRequest {
  id: string
  robotId: string
  label: string
  status: ActionStatus
  localReflex: boolean
  submittedAt: number
  resolvedAt: number | null
  votes: ActionVote[]
}

export interface SkeletonPose {
  positions: Record<string, { x: number; y: number }>
  bones: [string, string][]
  viewBox: string
}

export interface FlightBatch {
  batchHeight: number
  verdict: 'green' | 'red'
  timestamp: number
  certHash: string
  flaggedRobot: string | null
}

export interface FleetSimState {
  tick: number
  robots: RobotState[]
  operators: OperatorNode[]
  batches: FlightBatch[]
  actions: ActionRequest[]
}

const MORPHOLOGIES: Record<Morphology, { joints: JointDef[]; cols: number; rows: number }> = {
  biped: {
    cols: 3,
    rows: 6,
    joints: [
      { id: 'head_yaw', label: 'Head', row: 1, col: 2 },
      { id: 'shoulder_l', label: 'Shldr L', row: 2, col: 1 },
      { id: 'torso', label: 'Torso', row: 2, col: 2 },
      { id: 'shoulder_r', label: 'Shldr R', row: 2, col: 3 },
      { id: 'elbow_l', label: 'Elbow L', row: 3, col: 1 },
      { id: 'elbow_r', label: 'Elbow R', row: 3, col: 3 },
      { id: 'hip_l', label: 'Hip L', row: 4, col: 1 },
      { id: 'hip_r', label: 'Hip R', row: 4, col: 3 },
      { id: 'knee_l', label: 'Knee L', row: 5, col: 1 },
      { id: 'knee_r', label: 'Knee R', row: 5, col: 3 },
      { id: 'ankle_l', label: 'Ankle L', row: 6, col: 1 },
      { id: 'ankle_r', label: 'Ankle R', row: 6, col: 3 },
    ],
  },
  quadruped: {
    cols: 3,
    rows: 4,
    joints: [
      { id: 'head', label: 'Head', row: 1, col: 2 },
      { id: 'spine', label: 'Spine', row: 2, col: 2 },
      { id: 'leg_fl', label: 'Leg FL', row: 2, col: 1 },
      { id: 'leg_fr', label: 'Leg FR', row: 2, col: 3 },
      { id: 'leg_bl', label: 'Leg BL', row: 3, col: 1 },
      { id: 'leg_br', label: 'Leg BR', row: 3, col: 3 },
      { id: 'tail', label: 'Tail', row: 4, col: 2 },
    ],
  },
  arm: {
    cols: 1,
    rows: 5,
    joints: [
      { id: 'base', label: 'Base', row: 1, col: 1 },
      { id: 'shoulder', label: 'Shoulder', row: 2, col: 1 },
      { id: 'elbow', label: 'Elbow', row: 3, col: 1 },
      { id: 'wrist', label: 'Wrist', row: 4, col: 1 },
      { id: 'gripper', label: 'Gripper', row: 5, col: 1 },
    ],
  },
}

const FLEET_DEFS: { id: string; name: string; morphology: Morphology }[] = [
  { id: 'atlas-7', name: 'Atlas-7', morphology: 'biped' },
  { id: 'rover-3', name: 'Rover-3', morphology: 'quadruped' },
  { id: 'armunit-1', name: 'ArmUnit-1', morphology: 'arm' },
]

const OPERATOR_DEFS = ['op-mesa', 'op-cortez', 'op-nairobi', 'op-lyon', 'op-osaka']

const HISTORY_LEN = 60
const BATCH_LOG_LEN = 40
const ACTION_LOG_LEN = 25

// Emergency Stop bypasses gating entirely — it is the reflex-tier fail-safe
// (local, real-time, no consensus round-trip), not an intent-tier decision.
export const EMERGENCY_STOP = 'Emergency Stop'

export const ACTIONS_BY_MORPHOLOGY: Record<Morphology, string[]> = {
  biped: ['Walk Forward', 'Enter Room', 'Pick Up Object', 'Hand to Human', EMERGENCY_STOP],
  quadruped: ['Patrol Perimeter', 'Fetch Object', 'Sit', EMERGENCY_STOP],
  arm: ['Grasp Target', 'Release', 'Move to Waypoint', EMERGENCY_STOP],
}

export const SKELETON_POSES: Record<Morphology, SkeletonPose> = {
  biped: {
    viewBox: '0 0 100 140',
    positions: {
      head_yaw: { x: 50, y: 14 },
      torso: { x: 50, y: 42 },
      shoulder_l: { x: 30, y: 36 },
      shoulder_r: { x: 70, y: 36 },
      elbow_l: { x: 18, y: 62 },
      elbow_r: { x: 82, y: 62 },
      hip_l: { x: 40, y: 72 },
      hip_r: { x: 60, y: 72 },
      knee_l: { x: 37, y: 104 },
      knee_r: { x: 63, y: 104 },
      ankle_l: { x: 35, y: 132 },
      ankle_r: { x: 65, y: 132 },
    },
    bones: [
      ['head_yaw', 'torso'],
      ['torso', 'shoulder_l'],
      ['torso', 'shoulder_r'],
      ['shoulder_l', 'elbow_l'],
      ['shoulder_r', 'elbow_r'],
      ['torso', 'hip_l'],
      ['torso', 'hip_r'],
      ['hip_l', 'hip_r'],
      ['hip_l', 'knee_l'],
      ['hip_r', 'knee_r'],
      ['knee_l', 'ankle_l'],
      ['knee_r', 'ankle_r'],
    ],
  },
  quadruped: {
    viewBox: '0 0 100 110',
    positions: {
      head: { x: 50, y: 12 },
      spine: { x: 50, y: 50 },
      leg_fl: { x: 26, y: 38 },
      leg_fr: { x: 74, y: 38 },
      leg_bl: { x: 26, y: 82 },
      leg_br: { x: 74, y: 82 },
      tail: { x: 50, y: 98 },
    },
    bones: [
      ['head', 'spine'],
      ['spine', 'tail'],
      ['spine', 'leg_fl'],
      ['spine', 'leg_fr'],
      ['spine', 'leg_bl'],
      ['spine', 'leg_br'],
    ],
  },
  arm: {
    viewBox: '0 0 60 140',
    positions: {
      base: { x: 30, y: 12 },
      shoulder: { x: 30, y: 40 },
      elbow: { x: 30, y: 70 },
      wrist: { x: 30, y: 100 },
      gripper: { x: 30, y: 128 },
    },
    bones: [
      ['base', 'shoulder'],
      ['shoulder', 'elbow'],
      ['elbow', 'wrist'],
      ['wrist', 'gripper'],
    ],
  },
}

function confidenceFor(v: Verdict): number {
  if (v === 'green') return 92 + Math.random() * 8
  if (v === 'amber') return 55 + Math.random() * 20
  return 15 + Math.random() * 25
}

function nextVerdict(current: Verdict): Verdict {
  const r = Math.random()
  if (current === 'green') {
    if (r < 0.04) return 'amber'
    return 'green'
  }
  if (current === 'amber') {
    if (r < 0.2) return 'red'
    if (r < 0.6) return 'green'
    return 'amber'
  }
  // red — J-Lens gate has already blocked it; resolve quickly
  if (r < 0.7) return 'amber'
  return 'red'
}

function makeRobot(def: { id: string; name: string; morphology: Morphology }): RobotState {
  const shape = MORPHOLOGIES[def.morphology]
  const joints: JointNode[] = shape.joints.map((j) => ({
    ...j,
    verdict: 'green',
    confidence: confidenceFor('green'),
  }))
  return {
    id: def.id,
    name: def.name,
    morphology: def.morphology,
    cols: shape.cols,
    rows: shape.rows,
    joints,
    confidenceHistory: new Array(HISTORY_LEN).fill(96),
    health: 96,
  }
}

function hexByte(): string {
  return Math.floor(Math.random() * 256).toString(16).padStart(2, '0')
}

function fakeCertHash(): string {
  return `0x${Array.from({ length: 16 }, hexByte).join('')}`
}

function initState(): FleetSimState {
  return {
    tick: 0,
    robots: FLEET_DEFS.map(makeRobot),
    operators: OPERATOR_DEFS.map((id) => ({ id, label: id, agrees: true })),
    batches: [],
    actions: [],
  }
}

function stepActions(actions: ActionRequest[], robots: RobotState[]): ActionRequest[] {
  const stepped = actions.map((a): ActionRequest => {
    if (a.status === 'approved' || a.status === 'blocked') return a
    if (a.status === 'pending') {
      return { ...a, status: 'probing' }
    }
    // probing -> resolve. Healthier robots (fewer flagged joints right now)
    // clear gating more easily — this is what ties action-gating back to the
    // live joint verdicts instead of being an independent coin flip.
    const robot = robots.find((r) => r.id === a.robotId)
    const healthFactor = (robot?.health ?? 80) / 100
    const votes: ActionVote[] = OPERATOR_DEFS.map((operatorId) => ({
      operatorId,
      agrees: Math.random() < 0.5 + healthFactor * 0.45,
    }))
    const agreeCount = votes.filter((v) => v.agrees).length
    const approved = agreeCount / votes.length >= 0.6
    return {
      ...a,
      status: approved ? 'approved' : 'blocked',
      votes,
      resolvedAt: Date.now(),
    }
  })
  return stepped.slice(-ACTION_LOG_LEN)
}

function stepRobot(robot: RobotState, injectAnomaly: boolean): RobotState {
  const anomalyIdx = injectAnomaly ? Math.floor(Math.random() * robot.joints.length) : -1
  const joints = robot.joints.map((j, idx) => {
    const verdict = idx === anomalyIdx ? 'amber' : nextVerdict(j.verdict)
    return { ...j, verdict, confidence: confidenceFor(verdict) }
  })
  const avgConfidence = joints.reduce((sum, j) => sum + j.confidence, 0) / joints.length
  const history = [...robot.confidenceHistory.slice(1), Math.round(avgConfidence)]
  return {
    ...robot,
    joints,
    confidenceHistory: history,
    health: Math.round(avgConfidence),
  }
}

function stepFleet(prev: FleetSimState): FleetSimState {
  const tick = prev.tick + 1
  // Roughly every ~15 ticks, force a visible anomaly on a random robot so
  // the gate's blocking behaviour is actually observable, not just noise.
  const injectOn = tick % 15 === 0 ? Math.floor(Math.random() * prev.robots.length) : -1
  const robots = prev.robots.map((r, i) => stepRobot(r, i === injectOn))

  const anyRed = robots.some((r) => r.joints.some((j) => j.verdict === 'red'))
  const consensusVerdict: 'green' | 'red' = anyRed ? 'red' : 'green'
  const flaggedRobot = anyRed
    ? robots.find((r) => r.joints.some((j) => j.verdict === 'red'))?.name ?? null
    : null

  const operators = prev.operators.map((op) => ({
    ...op,
    agrees: Math.random() < 0.92,
  }))

  const batch: FlightBatch = {
    batchHeight: tick,
    verdict: consensusVerdict,
    timestamp: Date.now(),
    certHash: fakeCertHash(),
    flaggedRobot,
  }
  const batches = [...prev.batches, batch].slice(-BATCH_LOG_LEN)
  const actions = stepActions(prev.actions, robots)

  return { tick, robots, operators, batches, actions }
}

export function useFleetSimulation(intervalMs = 900) {
  const [history, setHistory] = useState<FleetSimState[]>(() => [initState()])
  const [viewedIndex, setViewedIndex] = useState(0)
  const liveRef = useRef(true)

  useEffect(() => {
    const id = setInterval(() => {
      setHistory((prev) => {
        const next = stepFleet(prev[prev.length - 1])
        const trimmed = [...prev, next].slice(-HISTORY_LEN)
        return trimmed
      })
      if (liveRef.current) {
        setViewedIndex((_i) => -1) // -1 sentinel resolved below to "latest"
      }
    }, intervalMs)
    return () => clearInterval(id)
  }, [intervalMs])

  const isLive = viewedIndex === -1 || viewedIndex >= history.length - 1
  const resolvedIndex = isLive ? history.length - 1 : viewedIndex
  const state = history[resolvedIndex] ?? history[history.length - 1]

  const scrub = (index: number) => {
    liveRef.current = index >= history.length - 1
    setViewedIndex(index)
  }

  const goLive = () => {
    liveRef.current = true
    setViewedIndex(-1)
  }

  const submitAction = (robotId: string, label: string) => {
    const isEmergencyStop = label === EMERGENCY_STOP
    setHistory((prev) => {
      const last = prev[prev.length - 1]
      const request: ActionRequest = {
        id: crypto.randomUUID(),
        robotId,
        label,
        // Emergency Stop is the reflex-tier fail-safe: local, instant,
        // never gated by consensus.
        status: isEmergencyStop ? 'approved' : 'pending',
        localReflex: isEmergencyStop,
        submittedAt: Date.now(),
        resolvedAt: isEmergencyStop ? Date.now() : null,
        votes: [],
      }
      const updatedLast: FleetSimState = {
        ...last,
        actions: [...last.actions, request].slice(-ACTION_LOG_LEN),
      }
      return [...prev.slice(0, -1), updatedLast]
    })
  }

  return {
    state,
    history,
    viewedIndex: resolvedIndex,
    isLive,
    scrub,
    goLive,
    submitAction,
  }
}
