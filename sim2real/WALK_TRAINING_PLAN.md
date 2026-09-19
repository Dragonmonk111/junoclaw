# Ponyu Walk Training Plan

## Current State (Sep 2026)

### Policies Trained
| Policy | Env | Steps | Reward Mean | Notes |
|--------|-----|-------|-------------|-------|
| `ppo_walk` | DogzillaWalkEnv | 500K | ~800 | Basic forward walk |
| `ppo_turn` | DogzillaTurnEnv | 500K | ~600 | Turning only |
| `ppo_speed_heading` | DogzillaSpeedHeadingEnv | 1M | ~1200 | Commandable speed+heading |
| `ppo_speed_heading_terrain` | DogzillaSpeedHeadingTerrainEnv | 2M | ~2800 | + terrain perturbation |
| `ppo_smooth_walk` | DogzillaSmoothWalkEnv | 2M | ~3500 | Track 2: smoother gait |

### Gait Parameters
| Parameter | Original | Smooth Walk | Effect |
|-----------|----------|-------------|--------|
| Gait freq | 4.0 Hz | 2.5 Hz | Less jerky, slower cadence |
| Lift angle | 0.30 rad | 0.18 rad | Gentler foot lift |
| Stride | 0.15 rad | 0.18 rad | Longer, smoother steps |
| Terrain rough | 0.015 | 0.008 | Gentler perturbations |
| Imitation weight | 0.5 | 1.5 | Tighter gait tracking |
| Energy penalty | 0.001 | 0.003 | Discourages jerky motion |
| Action smoothness | none | 0.05×Δ² | Penalizes step-to-step jitter |

## Track 2: Next Training Round

### Round 2A: Tuning for Carpet/Floor Transitions
**Goal**: Handle transitions between hard floor and carpet without stumbling.

```
python train_smooth_walk.py \
    --timesteps 3000000 \
    --n-envs 8 \
    --resume checkpoints/ppo_smooth_walk.zip \
    --roughness 0.012 \
    --lr 5e-5
```

Changes:
- Increase terrain roughness slightly (0.008 → 0.012)
- Lower learning rate (1e-4 → 5e-5) for fine-tuning
- 3M steps for better convergence

### Round 2B: Speed Variation Training
**Goal**: Walk at different speeds reliably (slow, medium, fast).

Add a new env variant that randomizes commanded speed during episodes:
- Speed range: 0.02 to 0.15 m/s
- Random speed changes every 200 steps
- Reward: track speed more tightly (exp(-v²/0.01))

### Round 2C: Outdoor Gravel/Grass
**Goal**: Handle outdoor surfaces.

- Add lateral terrain perturbation (simulating uneven ground)
- Add random friction changes (0.5× to 1.5×)
- Add random external forces (wind, bumps)
- Train for 5M steps

## Track 3: Advanced Locomotion (Future)

### 3A: Stair Climbing
- Already have `DogzillaStepEnv` and `ppo_step.zip`
- Need to retrain with smooth gait parameters
- Add stair height randomization (1-3 cm)

### 3B: Recovery from Pushes
- Already have `DogzillaRecoveryEnv` and `ppo_recovery_v5_2.zip`
- Integrate recovery policy as fallback when walk policy detects fall

### 3C: Sit-to-Stand Transition
- Train a transition policy that smoothly goes from sitting to standing
- Use the existing `DogzillaSitEnv` as starting point

## Hardware Testing Protocol

### Level 1: Bench Test (on desk, legs free)
1. `python3 run_on_pi.py --policy policy_smooth_walk.onnx --max-steps 50 --torque-limit 0.3`
2. Watch for smooth leg movements, no jittering
3. Check action smoothness in logs

### Level 2: Floor Test (on floor, supervised)
1. `python3 ponyou.py --cli walk` (4 second walk)
2. Watch for smooth gait, no stumbling
3. Test on hard floor and carpet
4. Test turn-left, turn-right

### Level 3: Voice Test (with child)
1. Start web server: `python3 ponyou.py`
2. Open on phone, test each button
3. Test voice commands
4. Test autonomy modes in safe area

### Level 4: Autonomy Test (open space)
1. Test explore mode in a room with obstacles
2. Test follow mode (walk in front of robot)
3. Test patrol mode in a square area
4. Test come-home mode

## Deployment Commands

```bash
# Export new policy
python export_onnx.py --checkpoint checkpoints/ppo_smooth_walk.zip --out checkpoints/policy_smooth_walk.onnx

# Copy to Pi
scp checkpoints/policy_smooth_walk.onnx checkpoints/policy_smooth_walk.onnx.data pi@10.42.0.1:~/
scp deploy/ponyou.py deploy/ponyou_vision.py deploy/ponyou_autonomy.py pi@10.42.0.1:~/

# On Pi
ssh pi@10.42.0.1
source /home/pi/RaspberryPi-CM5/xgovenv/bin/activate
pip install opencv-python-headless  # for vision
python3 ponyou.py --offsets /home/pi/xgo_offsets.json
```
