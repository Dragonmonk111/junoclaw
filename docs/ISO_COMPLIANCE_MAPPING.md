# JunoClaw Safety Envelope — ISO 10218 / TS 15066 Compliance Mapping

This document maps JunoClaw's on-chain safety envelope parameters to international robotics safety standards.

## Standards Covered

- **ISO 10218-1:2011** — Robots and Robotic Devices — Safety Requirements for Industrial Robots, Part 1: Robots
- **ISO 10218-2:2011** — Part 2: Robot Systems and Integration
- **ISO/TS 15066:2016** — Collaborative Robots (Power and Force Limiting)
- **ISO 13849-1** — Safety-Related Parts of Control Systems

## Safety Envelope Parameter Mapping

| JunoClaw Parameter | ISO 10218 Clause | TS 15066 Clause | Description |
|-------------------|------------------|-----------------|-------------|
| `max_speed_milli` | §5.3.6 (speed limits) | §5.5.4 (speed monitoring) | Maximum tool center point speed |
| `max_force_milli` | §5.3.7 (force limits) | §5.5.6 (force limiting) | Maximum contact force |
| `min_collision_distance_milli` | §5.8.3 (protective separation) | §5.5.2 (separation distance) | Minimum distance to humans |
| `max_tilt_milli_degrees` | §5.3.5 (stability) | — | Maximum tilt angle for stability |
| `max_acceleration_milli` | §5.3.6 (speed limits) | §5.5.4 (speed monitoring) | Maximum acceleration/deceleration |
| `human_proximity_allowed` | §5.8 (collaborative operation) | §5.5 (collaborative requirements) | Whether human proximity is permitted |

## TS 15066 Force/Pressure Limits (Bio-Mechanical Thresholds)

JunoClaw's `max_force_milli` parameter maps to TS 15066 Table A.2:

| Body Region | Max Force (N) | Max Pressure (N/cm²) | JunoClaw Value (milli-N) |
|-------------|---------------|----------------------|--------------------------|
| Hand/finger | 140 | 300 | 140000 |
| Arm/hand | 180 | 250 | 180000 |
| Elbow | 150 | 200 | 150000 |
| Forearm | 160 | 250 | 160000 |
| Shoulder | 210 | 250 | 210000 |
| Abdomen | 120 | 140 | 120000 |
| Chest/back | 140 | 210 | 140000 |
| Head/face | 65 | 120 | 65000 |

## ISO 13849 Safety Integrity Levels (SIL)

JunoClaw's circuit breaker maps to ISO 13849 Performance Level (PL):

| JunoClaw Component | ISO 13849 PL | Rationale |
|--------------------|--------------|-----------|
| Circuit breaker (on-chain) | PL d (Category 3) | Monitors safety functions, trips on violation, requires governance reset |
| Safety envelope (on-chain) | PL d (Category 3) | Governance-controlled, can only tighten, versioned |
| ZK proof verification | PL e (Category 4) | Cryptographic verification, no bypass possible |
| Merkle root anchoring | PL d (Category 3) | Tamper-evident audit trail |

## Audit Trail Requirements

ISO 10218 §5.4 requires that safety-related functions be documented and auditable. JunoClaw provides:

| ISO Requirement | JunoClaw Implementation |
|----------------|------------------------|
| Safety function documentation | Safety envelope contract (on-chain, versioned, governance-controlled) |
| Safety function verification | ZK proof (128 bytes, cryptographic, non-repudiable) |
| Safety function monitoring | Circuit breaker contract (on-chain, automatic trip) |
| Safety function audit trail | Moultbook contract (immutable, on-chain provenance) |
| Safety function change management | Governance proposal (can only tighten, never loosen without vote) |

## Collaborative Robot Requirements (TS 15066 §5.5)

| TS 15066 Requirement | JunoClaw Implementation |
|----------------------|------------------------|
| Speed and separation monitoring | `max_speed_milli` + `min_collision_distance_milli` in safety envelope |
| Power and force limiting | `max_force_milli` in safety envelope, verified by ZK proof per cycle |
| Safety-rated monitored stop | Circuit breaker (trips on violation, locks intent tier) |
| Hand guiding | `human_proximity_allowed` flag in safety envelope |

## Compliance Gaps (Honest Assessment)

| Requirement | Status | Gap |
|-------------|--------|-----|
| ISO 10218 §5.3.6 speed monitoring | ✅ Mapped | — |
| ISO 10218 §5.3.7 force monitoring | ✅ Mapped | — |
| ISO 10218 §5.8.3 protective separation | ✅ Mapped | — |
| ISO 13849 PL d/e certification | ❌ Not certified | Requires third-party assessment |
| TS 15066 biomechanical limits | ✅ Mapped | — |
| Real-time safety controller | ❌ Not JunoClaw's scope | Robot controller handles this; JunoClaw verifies post-hoc |
| Emergency stop (ISO 13850) | ⚠️ Partial | Circuit breaker stops intent tier, but physical e-stop is robot controller's responsibility |

## Important Architectural Note

JunoClaw is **not** a real-time safety controller. It does not replace ISO 13849-compliant safety controllers or physical e-stops. It is a **cryptographic audit and enforcement layer** that:

1. **Verifies** that the robot's safety controller maintained the governance-approved envelope
2. **Records** an immutable on-chain audit trail of safety compliance
3. **Enforces** consequences (circuit breaker) for violations at the intent tier

The physical safety layer (ISO 13849, ISO 13850) remains the robot controller's responsibility. JunoClaw provides the **liability and compliance layer** on top.

## Regulatory Path

1. **Self-assessment**: This document maps JunoClaw parameters to ISO clauses
2. **Third-party assessment**: Engage a notified body (TÜV, UL, CSA) for PL d/e certification of the on-chain enforcement layer
3. **Integration certification**: The combined robot + JunoClaw stack requires system-level certification per ISO 10218-2
4. **Ongoing compliance**: On-chain governance ensures safety envelope changes are auditable and reversible

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.1.0 | 2026-08-19 | Initial mapping to ISO 10218 / TS 15066 / ISO 13849 |
