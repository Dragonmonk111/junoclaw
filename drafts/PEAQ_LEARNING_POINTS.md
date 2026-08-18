# PeaqOS for Unitree — Learning Points for Juno Agents DAO

Source: peaq docs, peaq-robotics-ros2 SDK, peaqOS for Unitree demo, peaq Escrow (ERC-8004 + LayerZero)

## What peaq Does Well

1. **Machine Identity (peaqID / W3C DID)**
   - One DID per machine, one per proxy operator
   - DID resolves to key-value attribute store on peaq chain
   - Attributes: `machineId`, `nftTokenId`, `operator`, `documentation_url`, `data_api`, `data_visibility`
   - This solves the "anonymous hardware" problem — robots have a verifiable on-chain identity and track record

2. **Data Visibility Modes**
   - `public`: MCR API fetches raw data from `data_api`
   - `private`: MCR API returns `data_api` URL, consumer fetches directly
   - `onchain`: Raw event data is parsed from EventRegistry events on-chain
   - This is a useful design pattern for Juno — our coordination-settler already stores commitments on-chain, with plaintext mirrored off-chain

3. **ROS 2 + Blockchain Integration**
   - peaq provides a ROS 2 node wrapping peaqOS
   - Services: identity create/read, storage add/read, access control, event validation
   - Tether WDK for USDT balances, storage bridge with Pinata/IPFS
   - This proves robotics + blockchain integration is production-feasible

4. **peaq Escrow (ERC-8004 + LayerZero)**
   - Machines offer/consume real-world services cross-chain
   - Buyer and seller lock stakes, work is performed off-chain, payment released after verification
   - Uses ERC-8004 standard for "trusted agents" — machine reputation recorded on-chain
   - Cross-chain messaging via LayerZero, but machines never leave peaq

5. **Event Validation on peaq**
   - `PeaqosValidateEvent` takes `machine_id`, `event_type`, `value`, `timestamp`, `raw_data_hex`, `trust_level`, `source_chain_id`, `source_tx_hash`, `metadata_hex`
   - Events are batched and submitted on-chain
   - This is similar in spirit to our `coordination-settler` batch submissions, but peaq's trust level is a parameter passed by the caller, not a cryptographically proven internal state

## Where peaq Does NOT Solve Verifiable Trust

peaq gives robots **verifiable identity, track record, and settlement**. It does NOT give you **verifiable internal state** or **deterministic truth** about whether the agent actually did the work.

Key gaps:

1. **No internal-state verification:** peaq's `trust_level` is an input parameter. It is not a cryptographic proof that the agent's model actually processed the sensor data. A robot can submit `trust_level: 1` while doing nothing.

2. **No anti-hallucination gate:** The robot's claim "I inspected the pipeline" is stored on-chain as an event, but the chain cannot verify the claim is true. It only verifies the robot's identity signed the event.

3. **No Jacobian / internal-state probing:** peaq verifies signatures and identities. It does not probe the agent's neural network to confirm the output was produced by genuine computation.

4. **Sensor data not verified:** Even if raw sensor data is uploaded to IPFS, the chain cannot verify the data was not tampered with before upload. peaq relies on the robot's local wallet to sign.

## What This Means for Our Stack

peaq proves the market need (robot identity, service discovery, escrow, cross-chain settlement). Our stack can complement peaq by adding the missing layer:

- **peaq handles:** machine identity, service marketplace, escrow, cross-chain settlement
- **Juno / J-Lens handles:** verifiable agent output, anti-hallucination gating, deterministic trust scoring, threshold consensus on whether the agent's claim is true

In other words: peaq tells you *which robot* did the work and *when* it was paid. J-Lens tells you *whether the robot's claim about the work is true*.

## Question: Is a Robot Verifiable and Deterministic with Chain Metadata + Jacobian Truth Probing?

**Short answer:** Partially — and the partial parts matter.

### What chain metadata gives you

Chain metadata (DID, event history, status reports, escrow state) gives you:
- **Verifiable identity:** The robot is who it says it is.
- **Verifiable track record:** Past events are signed and timestamped.
- **Verifiable settlement:** Payments and escrow states are on-chain.

But chain metadata does **NOT** give you:
- Proof the robot actually performed the claimed action.
- Proof the agent's internal state matched its output.
- Protection against a robot that signs fabricated events.

### What Jacobian truth probing gives you

Jacobian truth probing (J-Lens) reads the agent's internal neural state and compares it to the output. This gives you:
- **Verifiable computation:** The output was produced by genuine internal processing, not generated from a template or stolen.
- **Anti-hallucination:** If the model's internal state is inconsistent with the claimed action, J-Lens flags red.
- **Deterministic trust:** Same internal state pattern always produces the same verdict.

But J-Lens probing alone is **NOT enough** for full robot verifiability because:

1. **It verifies the agent, not the sensor hardware.** J-Lens can prove the model "believes" it saw a pipeline crack. It cannot prove the camera actually captured the image unless the sensor is tampered-proof (TEE, secure enclave, trusted oracle).

2. **It does not verify physical actuation.** J-Lens can verify the agent decided to "tighten bolt #3." It cannot verify the motor actually turned unless the actuator has a TEE-attested feedback loop.

3. **It assumes the model is the one processing the input.** If the input was replaced before it reached the model (adversarial sensor spoofing), J-Lens will verify the model's processing of a fake input.

### Full Deterministic Robotics Stack

For full verifiability, you need three layers working together:

```
Layer 1: SENSORS / ACTUATORS (TEE-attested)
   - Camera, lidar, motor encoders run in a TEE
   - Raw sensor data is signed by the hardware chip
   - This proves the input is authentic and untampered

Layer 2: AGENT / MODEL (J-Lens + Jacobian probing)
   - Model processes sensor data
   - J-Lens reads internal states, produces green/yellow/red verdict
   - This proves the model genuinely "thought through" the decision

Layer 3: COORDINATION / SETTLEMENT (Juno + Commonware P2P)
   - Multiple validators agree on the ordered batch of commands/observations
   - Juno stores the threshold certificate and proof
   - This proves consensus and provides an audit trail
```

**Conclusion:** A robot is *not* fully verifiable and deterministic with chain metadata alone. It is also *not* fully verifiable with Jacobian probing alone. The combination of **TEE-attested sensors**, **J-Lens internal-state probing**, and **on-chain settlement** is what gets you close to deterministic trust in robotics.

## Tactical Note for A48

Mentioning peaq in the proposal shows we understand the market. The pitch is:

> "peaq is building the identity, marketplace, and escrow layer for robots. We are building the truth and coordination layer that verifies the robot actually did the work before payment is released. Our stack is complementary, not competitive."
