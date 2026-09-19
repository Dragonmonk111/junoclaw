"""Ponyou — the first JunoClaw robot agent.

A web-based command interface for the DOGZILLA-Lite robot. Runs on the Pi.
Open http://10.42.0.1:8080 from a phone browser to control the robot.

Phase 1: Button commands (stand, sit, walk, stop, turn, wave)
Phase 2: Voice commands via Web Speech API (talk to Ponyu!)
Phase 3: Natural language understanding + personality + TTS (Ponyu talks back!)

Usage on the robot:
    source /home/pi/RaspberryPi-CM5/xgovenv/bin/activate
    python3 ponyou.py --offsets /home/pi/xgo_offsets.json

Usage from CLI (no web):
    python3 ponyou.py --cli --offsets /home/pi/xgo_offsets.json walk
    python3 ponyou.py --cli --offsets /home/pi/xgo_offsets.json stand

Chat from CLI:
    python3 ponyou.py --chat --offsets /home/pi/xgo_offsets.json "hey ponyou walk forward"
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import sys
import threading
import time
from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs

import numpy as np

from xgo_robot import XGORobot, load_offsets, save_offsets, SIM_NEUTRAL_RAD
from run_on_pi import SafetyController, run_policy

try:
    import onnxruntime as ort
except ImportError:
    ort = None

try:
    from ponyou_vision import PonyuVision
except ImportError:
    PonyuVision = None

try:
    from ponyou_autonomy import PonyuAutonomy
except ImportError:
    PonyuAutonomy = None


# ---------------------------------------------------------------------------
# Ponyou personality
# ---------------------------------------------------------------------------

import random
import re

PONYOU_NAME = "Ponyou"
PONYOU_GREETING = f"Hi! I'm {PONYOU_NAME}. I can stand, sit, walk, and wave!"

COMMANDS = {
    "stand":      "Stand up tall",
    "sit":        "Sit down",
    "prone":      "Lie down",
    "walk":       "Walk forward",
    "stop":       "Stop and stand",
    "turn-left":  "Turn left",
    "turn-right": "Turn right",
    "wave":       "Wave hello",
    "hello":      f"Say hi (I'm {PONYOU_NAME}!)",
    "battery":    "Check battery",
    "dance":      "Do a little dance",
    "explore":    "Explore autonomously",
    "follow":     "Follow me",
    "patrol":     "Patrol the area",
    "come-home":  "Come back home",
}

# Built-in XGO actions (if available on the firmware)
XGO_BUILTIN = {
    "stand": "stand",
    "sit": "sit",
    "prone": "prone",
    "wave": "wave",
    "dance": "dance",
}

# ---------------------------------------------------------------------------
# Phase 3: PonyuBrain — natural language understanding + personality
# ---------------------------------------------------------------------------

# Keyword → command mapping for offline NLU
_NLU_RULES = [
    # walk
    (r"\b(walk|go|move|forward|ahead|step|walk\s+forward|let.?s go|come here|come on)\b", "walk"),
    # stop
    (r"\b(stop|halt|wait|freeze|hold|stay|don.?t move|whoa)\b", "stop"),
    # stand
    (r"\b(stand|get\s+up|rise|up|stand\s+up|on\s+your\s+feet)\b", "stand"),
    # sit
    (r"\b(sit|sit\s+down|rest|take\s+a\s+break|chill|relax)\b", "sit"),
    # prone / lie down
    (r"\b(lie|lay|prone|sleep|tired|nap|bed\s+time|lie\s+down|lay\s+down)\b", "prone"),
    # turn left
    (r"\b(left|turn\s+left|go\s+left|spin\s+left)\b", "turn-left"),
    # turn right
    (r"\b(right|turn\s+right|go\s+right|spin\s+right)\b", "turn-right"),
    # wave
    (r"\b(wave|hi|hey|hello|yo|sup|greet|say\s+hi|wave\s+hello)\b", "wave"),
    # dance
    (r"\b(dance|boogie|groove|party|show\s+off|trick)\b", "dance"),
    # battery
    (r"\b(battery|power|energy|charge|tired|how\s+much\s+power|low\s+power)\b", "battery"),
    # name / identity
    (r"\b(your\s+name|who\s+are\s+you|what.?s\s+your\s+name|ponyu)\b", "hello"),
    # explore
    (r"\b(explore|adventure|wander|roam|look\s+around|go\s+explore)\b", "explore"),
    # follow
    (r"\b(follow|come\s+with|follow\s+me|come\s+along|after\s+me)\b", "follow"),
    # patrol
    (r"\b(patrol|guard|watch|secure|rounds)\b", "patrol"),
    # come home
    (r"\b(come\s+home|come\s+back|return|come\s+here|back|home)\b", "come-home"),
]

# Personality responses — Ponyu is playful, friendly, and a bit goofy
_RESPONSES = {
    "walk": [
        "Let's go! Woohoo!",
        "Onward! I love walking!",
        "Here I come!",
        "Step step step, here we go!",
    ],
    "stop": [
        "Okay, stopping!",
        "Whoa there! I'm stopped.",
        "Halt! Standing still like a statue.",
        "Okay okay, I'll wait here.",
    ],
    "stand": [
        "Standing tall and proud!",
        "Up and at 'em!",
        "I'm up! What's next?",
        "Standing like a good robot!",
    ],
    "sit": [
        "Ooh, a break! Sitting down.",
        "Ah, that's nice. Sitting.",
        "Sitting down! I like this.",
        "Okay, I'll sit here for a bit.",
    ],
    "prone": [
        "Aww, nap time? Okay...",
        "Lying down. Don't step on me!",
        "Zzz... oh wait, I'm a robot. Lying down!",
        "Flat like a pancake!",
    ],
    "turn-left": [
        "Turning left! Wheee!",
        "Left we go!",
        "Spinning left!",
    ],
    "turn-right": [
        "Turning right! Wheee!",
        "Right we go!",
        "Spinning right!",
    ],
    "wave": [
        "Hi there! *waves*",
        "Hello hello! Nice to see you!",
        "Hey! I'm Ponyu! *waves excitedly*",
        "Waving hi! Do you want to play?",
    ],
    "dance": [
    "Time to dance! *does a little shimmy*",
    "Watch my moves! *wiggles*",
    "Dance time! I'm the best dancing robot!",
    ],
    "hello": [
        f"Hi! I'm {PONYOU_NAME}! I'm a robot dog. I can walk, sit, wave, and dance!",
        f"Hello! My name is {PONYOU_NAME}. Want to play with me?",
        f"Hey there! I'm {PONYOU_NAME}, the coolest robot dog around!",
    ],
    "battery": [
        "Let me check my energy...",
        "Checking my battery! Beep boop.",
        "How much power do I have? Let me see...",
    ],
    "explore": [
        "Adventure time! Let's go explore!",
        "Exploring mode activated! I'll avoid obstacles!",
        "On an adventure! Watch me roam around!",
    ],
    "follow": [
        "I'll follow you! Let's go together!",
        "Following you! Don't walk too fast!",
        "I'm your little shadow! Following along!",
    ],
    "patrol": [
        "Patrol mode! I'm guarding the area!",
        "On patrol! I'll walk in a square!",
        "Guard duty! Watching over everything!",
    ],
    "come-home": [
        "Coming back home!",
        "I'll try to find my way back!",
        "Heading back! I miss my spot!",
    ],
    "unknown": [
        "Hmm, I don't know that one yet. Try saying walk, sit, stand, wave, explore, or follow!",
        "I'm still learning! Try: walk, stop, sit, stand, turn, wave, dance, explore, or follow!",
        "I don't understand that yet. But I can walk, sit, stand, wave, dance, explore, and follow!",
    ],
}


class PonyuBrain:
    """Ponyu's brain: maps natural language to commands and generates responses."""

    def __init__(self):
        self._history = []

    def understand(self, text: str) -> tuple[str | None, str]:
        """Parse natural language text into a command + response.

        Returns (command, response) where command may be None if not understood.
        """
        text_lower = text.lower().strip()
        self._history.append({"role": "user", "text": text})

        command = None
        for pattern, cmd in _NLU_RULES:
            if re.search(pattern, text_lower):
                command = cmd
                break

        if command is None:
            response = random.choice(_RESPONSES["unknown"])
            self._history.append({"role": "ponyu", "text": response})
            return None, response

        response = random.choice(_RESPONSES.get(command, _RESPONSES["unknown"]))
        self._history.append({"role": "ponyu", "text": response})
        return command, response

    def greeting(self) -> str:
        return random.choice(_RESPONSES["hello"])

    def battery_response(self, pct: int) -> str:
        if pct > 70:
            return f"I'm at {pct}% battery! Full of energy! Let's play!"
        elif pct > 30:
            return f"I'm at {pct}% battery. I've got plenty of play time left!"
        else:
            return f"I'm at {pct}% battery... getting a bit tired. But I can still play!"


class PonyuController:
    """Controls the robot via XGO built-in commands and RL policies."""

    def __init__(self, robot: XGORobot, offsets_path: str):
        self.robot = robot
        self.offsets_path = offsets_path
        self._busy = False
        self._lock = threading.Lock()
        self._policy_cache = {}
        self.brain = PonyuBrain()
        self.vision = None
        self.autonomy = None

        # Try to start vision system (camera)
        if PonyuVision is not None:
            self.vision = PonyuVision(camera_index=0)
            if self.vision.start():
                print(f"[Ponyou] Vision: camera active")
            else:
                print(f"[Ponyou] Vision: no camera found (vision features disabled)")
                self.vision = None
        else:
            print(f"[Ponyou] Vision: ponyou_vision module not available")

        # Initialize autonomy controller
        if PonyuAutonomy is not None:
            self.autonomy = PonyuAutonomy(self, self.vision)
        else:
            print(f"[Ponyou] Autonomy: ponyou_autonomy module not available")

        battery = robot.read_battery()
        print(f"[Ponyou] Battery: {battery}%")
        print(f"[Ponyou] Firmware: {robot.read_firmware()}")
        print(f"[Ponyou] Ready! Open http://10.42.0.1:8080 on your phone.")
        print(f"[Ponyou] Voice: tap the mic button and talk to me!")
        if self.vision:
            print(f"[Ponyou] Camera feed: http://10.42.0.1:8080/video")

    def _get_policy_path(self, name: str) -> str:
        """Find policy file in home dir or checkpoints."""
        candidates = [
            os.path.expanduser(f"~/{name}"),
            os.path.expanduser(f"~/checkpoints/{name}"),
            os.path.join(os.path.dirname(__file__), "checkpoints", name),
        ]
        for p in candidates:
            if os.path.exists(p):
                return p
        return None

    def execute(self, command: str, duration: float = 4.0) -> dict:
        """Execute a command. Returns result dict."""
        if command not in COMMANDS:
            return {"ok": False, "error": f"Unknown command: {command}"}

        with self._lock:
            if self._busy:
                return {"ok": False, "error": "Busy, wait for current action to finish"}
            self._busy = True

        try:
            result = self._dispatch(command, duration)
            return result
        except Exception as e:
            return {"ok": False, "error": str(e)}
        finally:
            with self._lock:
                self._busy = False

    def chat(self, text: str) -> dict:
        """Process natural language text, execute the understood command, and respond."""
        command, response = self.brain.understand(text)

        if command is None:
            return {"ok": True, "command": None, "response": response}

        if command == "battery":
            pct = int(self.robot.read_battery())
            return {"ok": True, "command": "battery", "response": self.brain.battery_response(pct), "battery": pct}

        if command == "hello":
            return {"ok": True, "command": "hello", "response": response, "battery": int(self.robot.read_battery())}

        result = self.execute(command)
        result["response"] = response
        result["command"] = command
        return result

    def execute_autonomy(self, mode: str, duration: float = 60.0) -> dict:
        """Start an autonomous mode."""
        if self.autonomy is None:
            return {"ok": False, "error": "Autonomy module not available"}
        if mode not in ("explore", "follow", "patrol", "come-home"):
            return {"ok": False, "error": f"Unknown mode: {mode}"}
        result = self.autonomy.start_mode(mode, duration)
        return result

    def stop_autonomy(self) -> dict:
        """Stop autonomous mode."""
        if self.autonomy is None:
            return {"ok": False, "error": "Autonomy module not available"}
        self.autonomy.stop()
        return {"ok": True, "message": "Autonomy stopped"}

    def get_vision_status(self) -> dict:
        """Return vision + autonomy status."""
        status = {
            "vision": self.vision.get_status() if self.vision else None,
            "autonomy": self.autonomy.get_status() if self.autonomy else None,
        }
        return status

    def _dispatch(self, command: str, duration: float) -> dict:
        if command == "hello":
            return {"ok": True, "message": PONYOU_GREETING, "battery": int(self.robot.read_battery())}

        if command == "battery":
            return {"ok": True, "battery": int(self.robot.read_battery())}

        if command == "stop":
            self.robot.reset_pose()
            return {"ok": True, "message": "Stopped"}

        # Built-in XGO actions (implemented via direct motor commands)
        if command == "sit":
            return self._sit()
        if command == "prone":
            return self._prone()
        if command == "dance":
            return self._dance()

        # RL policy actions
        if command == "walk":
            return self._run_walk_policy(speed=0.08, heading=0.0, duration=duration)

        if command == "turn-left":
            return self._run_walk_policy(speed=0.04, heading=0.3, duration=2.0)

        if command == "turn-right":
            return self._run_walk_policy(speed=0.04, heading=-0.3, duration=2.0)

        if command == "stand":
            self.robot.reset_pose()
            return self._run_stand_policy(duration=2.0)

        if command == "wave":
            return self._wave()

        # Autonomous modes
        if command in ("explore", "follow", "patrol", "come-home"):
            return self.execute_autonomy(command, duration=max(duration, 30.0))

        # Fallback: try XGO do() with the raw command
        try:
            self.robot.dog.do(command)
            time.sleep(1.5)
            return {"ok": True, "message": f"Did {command}"}
        except Exception as e:
            return {"ok": False, "error": f"Cannot do {command}: {e}"}

    def _run_walk_policy(self, speed: float, heading: float, duration: float) -> dict:
        # Prefer smooth walk policy (Track 2), fall back to terrain policy
        policy_path = self._get_policy_path("policy_smooth_walk.onnx")
        if not policy_path:
            policy_path = self._get_policy_path("policy_speed_heading_terrain.onnx")
        if not policy_path:
            return {"ok": False, "error": "Walk policy not found"}

        max_steps = int(duration * 50)  # 50 Hz
        safety = SafetyController(
            torque_limit=0.7,
            fall_threshold=0.3,
            action_smooth=0.1,
        )
        print(f"[Ponyou] Walk: speed={speed} heading={heading} steps={max_steps}")
        run_policy(
            policy_path=policy_path,
            robot=self.robot,
            safety=safety,
            obs_dim=43,
            speed_cmd=speed,
            heading_cmd=heading,
            max_steps=max_steps,
        )
        return {"ok": True, "message": f"Walked for {duration:.0f}s"}

    def _run_stand_policy(self, duration: float) -> dict:
        policy_path = self._get_policy_path("policy_stand.onnx")
        if not policy_path:
            self.robot.reset_pose()
            return {"ok": True, "message": "Stood up (hardware reset)"}

        max_steps = int(duration * 50)
        safety = SafetyController(
            torque_limit=0.3,
            fall_threshold=0.3,
            action_smooth=0.0,
        )
        run_policy(
            policy_path=policy_path,
            robot=self.robot,
            safety=safety,
            obs_dim=41,
            max_steps=max_steps,
        )
        return {"ok": True, "message": "Standing"}

    def _sit(self) -> dict:
        """Make the robot sit on its haunches."""
        try:
            # Front legs: keep relatively straight (slight bend)
            # Back legs: fold under (thigh up, calf folded back)
            # Motor IDs: 13=fl_hip 12=fl_thigh 11=fl_calf
            #            23=fr_hip 22=fr_thigh 21=fr_calf
            #            33=rl_hip 32=rl_thigh 31=rl_calf
            #            43=rr_hip 42=rr_thigh 41=rr_calf
            offsets = self.robot.offsets
            neutral = np.degrees(SIM_NEUTRAL_RAD) + offsets

            # Front legs: slight forward lean
            self.robot.dog.motor(12, neutral[1] + 10)  # fl_thigh forward
            self.robot.dog.motor(11, neutral[2] - 20)  # fl_calf
            self.robot.dog.motor(22, neutral[4] + 10)  # fr_thigh forward
            self.robot.dog.motor(21, neutral[5] - 20)  # fr_calf

            # Back legs: fold under (sit position)
            # Thigh rotates UP (knee rises), calf folds BACK (foot tucks under)
            self.robot.dog.motor(32, neutral[7] + 50)  # rl_thigh up
            self.robot.dog.motor(31, neutral[8] - 50)  # rl_calf fold back
            self.robot.dog.motor(42, neutral[10] + 50) # rr_thigh up
            self.robot.dog.motor(41, neutral[11] - 50) # rr_calf fold back

            time.sleep(1.5)
            return {"ok": True, "message": "Sitting down!"}
        except Exception as e:
            return {"ok": False, "error": f"Sit failed: {e}"}

    def _prone(self) -> dict:
        """Make the robot lie down flat."""
        try:
            offsets = self.robot.offsets
            neutral = np.degrees(SIM_NEUTRAL_RAD) + offsets

            # All legs splayed out, body low
            for thigh_id, calf_id, idx in [
                (12, 11, 1), (22, 21, 4), (32, 31, 7), (42, 41, 10)
            ]:
                self.robot.dog.motor(thigh_id, neutral[idx] + 30)  # thigh out
                self.robot.dog.motor(calf_id, neutral[idx + 1] + 40)  # calf flat

            time.sleep(1.5)
            return {"ok": True, "message": "Lying down!"}
        except Exception as e:
            return {"ok": False, "error": f"Prone failed: {e}"}

    def _dance(self) -> dict:
        """Do a little dance — shift weight side to side with arm waves."""
        try:
            offsets = self.robot.offsets
            neutral = np.degrees(SIM_NEUTRAL_RAD) + offsets

            for _ in range(3):
                # Lean left, arm right
                self.robot.dog.motor(13, neutral[0] + 15)  # fl_hip
                self.robot.dog.motor(23, neutral[3] + 15)  # fr_hip
                self.robot.dog.motor(52, 40)  # arm up
                time.sleep(0.4)

                # Lean right, arm left
                self.robot.dog.motor(13, neutral[0] - 15)
                self.robot.dog.motor(23, neutral[3] - 15)
                self.robot.dog.motor(52, -20)  # arm down
                time.sleep(0.4)

            # Return to neutral
            self.robot.reset_pose()
            return {"ok": True, "message": "Danced! Woohoo!"}
        except Exception as e:
            return {"ok": False, "error": f"Dance failed: {e}"}

    def _wave(self) -> dict:
        """Wave the arm gripper up and down."""
        try:
            # Arm motor IDs: 51=base, 52=shoulder, 53=gripper
            self.robot.dog.motor(52, 45)   # shoulder up
            time.sleep(0.5)
            self.robot.dog.motor(52, -30)  # shoulder down
            time.sleep(0.5)
            self.robot.dog.motor(52, 45)   # up again
            time.sleep(0.5)
            self.robot.dog.motor(52, 0)    # back to neutral
            time.sleep(0.3)
            return {"ok": True, "message": "Waved hello!"}
        except Exception as e:
            return {"ok": False, "error": f"Wave failed: {e}"}

    def is_busy(self) -> bool:
        with self._lock:
            return self._busy

    def cleanup(self):
        """Clean up vision and autonomy on shutdown."""
        if self.autonomy:
            self.autonomy.stop()
        if self.vision:
            self.vision.stop()


# ---------------------------------------------------------------------------
# Web UI
# ---------------------------------------------------------------------------

HTML_PAGE = """<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Ponyou 🐕</title>
<style>
  * { box-sizing: border-box; margin: 0; padding: 0; }
  body {
    font-family: -apple-system, sans-serif;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    min-height: 100vh; display: flex; align-items: center; justify-content: center;
  }
  .container { max-width: 440px; width: 92%; padding: 16px; }
  h1 { color: white; text-align: center; font-size: 2em; margin-bottom: 2px; }
  .subtitle { color: rgba(255,255,255,0.7); text-align: center; margin-bottom: 14px; font-size: 0.9em; }
  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }
  .btn {
    padding: 18px 10px; border: none; border-radius: 14px;
    font-size: 1em; font-weight: 600; cursor: pointer;
    transition: transform 0.1s, box-shadow 0.2s;
    box-shadow: 0 3px 12px rgba(0,0,0,0.2);
  }
  .btn:active { transform: scale(0.95); }
  .btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .btn-walk { background: #4CAF50; color: white; }
  .btn-stand { background: #2196F3; color: white; }
  .btn-sit { background: #FF9800; color: white; }
  .btn-stop { background: #f44336; color: white; grid-column: span 2; }
  .btn-turn { background: #9C27B0; color: white; }
  .btn-wave { background: #FF6B6B; color: white; }
  .btn-dance { background: #E91E63; color: white; }
  .btn-battery { background: #607D8B; color: white; }
  .btn-auto { background: #00838F; color: white; }
  .btn-follow { background: #2E7D32; color: white; }
  .btn-patrol { background: #4527A0; color: white; }
  .btn-home { background: #5D4037; color: white; }
  .section-label { color: rgba(255,255,255,0.6); font-size: 0.8em; margin: 14px 0 6px; text-transform: uppercase; letter-spacing: 1px; }

  /* Voice section */
  .voice-section { margin-top: 12px; display: flex; gap: 10px; align-items: center; }
  .mic-btn {
    width: 60px; height: 60px; border-radius: 50%; border: none;
    background: #FF4081; color: white; font-size: 1.6em; cursor: pointer;
    box-shadow: 0 3px 12px rgba(0,0,0,0.3); transition: transform 0.1s;
    flex-shrink: 0; display: flex; align-items: center; justify-content: center;
  }
  .mic-btn:active { transform: scale(0.9); }
  .mic-btn.recording { background: #f44336; animation: pulse 1s infinite; }
  @keyframes pulse { 0%{box-shadow:0 0 0 0 rgba(244,67,54,0.4)} 70%{box-shadow:0 0 0 15px rgba(244,67,54,0)} 100%{box-shadow:0 0 0 0 rgba(244,67,54,0)} }
  .chat-input {
    flex: 1; padding: 14px; border: none; border-radius: 14px;
    font-size: 1em; background: rgba(255,255,255,0.95); color: #333;
  }
  .send-btn {
    padding: 14px 18px; border: none; border-radius: 14px;
    background: #4CAF50; color: white; font-size: 1em; font-weight: 600;
    cursor: pointer; flex-shrink: 0;
  }

  /* Conversation */
  .chat-log {
    margin-top: 12px; max-height: 180px; overflow-y: auto;
    padding: 10px; border-radius: 12px; background: rgba(0,0,0,0.25);
  }
  .chat-msg { color: white; margin-bottom: 8px; font-size: 0.95em; line-height: 1.3; }
  .chat-msg .user { color: #80DEEA; font-weight: 600; }
  .chat-msg .ponyu { color: #FFD54F; font-weight: 600; }

  /* Camera */
  .camera-section { margin-top: 12px; }
  .camera-feed {
    width: 100%; border-radius: 12px; background: rgba(0,0,0,0.3);
    display: block; min-height: 120px;
  }

  .status {
    margin-top: 12px; padding: 12px; border-radius: 12px;
    background: rgba(0,0,0,0.3); color: white; text-align: center;
    font-size: 0.95em; min-height: 44px; display: flex;
    align-items: center; justify-content: center;
  }
  .battery-bar {
    margin-top: 8px; height: 20px; border-radius: 10px;
    background: rgba(255,255,255,0.2); overflow: hidden;
  }
  .battery-fill { height: 100%; border-radius: 10px; transition: width 0.5s; }
  .tts-toggle { margin-top: 8px; text-align: center; }
  .tts-toggle label { color: rgba(255,255,255,0.7); font-size: 0.85em; cursor: pointer; }
</style>
</head>
<body>
<div class="container">
  <h1>🐕 Ponyou</h1>
  <p class="subtitle">JunoClaw Robot Agent — tap buttons or talk to me!</p>

  <div class="grid">
    <button class="btn btn-walk" onclick="cmd('walk')">🚶 Walk</button>
    <button class="btn btn-stand" onclick="cmd('stand')">🧍 Stand</button>
    <button class="btn btn-sit" onclick="cmd('sit')">🪑 Sit</button>
    <button class="btn btn-turn" onclick="cmd('turn-left')">↪️ Left</button>
    <button class="btn btn-turn" onclick="cmd('turn-right')">↩️ Right</button>
    <button class="btn btn-wave" onclick="cmd('wave')">👋 Wave</button>
    <button class="btn btn-dance" onclick="cmd('dance')">� Dance</button>
    <button class="btn btn-battery" onclick="cmd('battery')">🔋 Battery</button>
  </div>

  <p class="section-label">Autonomy</p>
  <div class="grid">
    <button class="btn btn-auto" onclick="cmd('explore')">🧭 Explore</button>
    <button class="btn btn-follow" onclick="cmd('follow')">👤 Follow Me</button>
    <button class="btn btn-patrol" onclick="cmd('patrol')">🛡️ Patrol</button>
    <button class="btn btn-home" onclick="cmd('come-home')">🏠 Come Home</button>
  </div>

  <div class="voice-section">
    <button class="mic-btn" id="micBtn" onclick="toggleVoice()">🎤</button>
    <input type="text" class="chat-input" id="chatInput" placeholder="Type or talk to Ponyu..."
           onkeydown="if(event.key==='Enter')sendChat()">
    <button class="send-btn" onclick="sendChat()">Send</button>
  </div>

  <div class="chat-log" id="chatLog"></div>

  <div class="camera-section">
    <p class="section-label">Camera Feed</p>
    <img class="camera-feed" id="cameraFeed" src="" alt="No camera"
         onerror="this.style.display='none'" style="display:none">
  </div>

  <div class="tts-toggle">
    <label><input type="checkbox" id="ttsEnabled" checked onchange="saveTTS()"> 🔊 Ponyu talks back</label>
  </div>

  <button class="btn btn-stop" onclick="stopAll()">⏹️ STOP EVERYTHING</button>

  <div class="status" id="status">Ready! Tap a button or talk to Ponyu.</div>
  <div class="battery-bar"><div class="battery-fill" id="battery-fill" style="width:0%; background:#4CAF50;"></div></div>
</div>

<script>
let recognition = null;
let isRecording = false;

// --- TTS (Ponyu speaks) ---
function speak(text) {
  if (!document.getElementById('ttsEnabled').checked) return;
  if (!window.speechSynthesis) return;
  window.speechSynthesis.cancel();
  const u = new SpeechSynthesisUtterance(text);
  u.rate = 1.1; u.pitch = 1.3; u.volume = 0.8;
  const voices = window.speechSynthesis.getVoices();
  if (voices.length > 0) {
    const preferred = voices.find(v => v.lang.startsWith('en') && v.name.includes('Female'))
                  || voices.find(v => v.lang.startsWith('en'))
                  || voices[0];
    if (preferred) u.voice = preferred;
  }
  window.speechSynthesis.speak(u);
}
function saveTTS() { localStorage.setItem('ponyu_tts', document.getElementById('ttsEnabled').checked); }
function loadTTS() { const s = localStorage.getItem('ponyu_tts'); if (s !== null) document.getElementById('ttsEnabled').checked = s === 'true'; }

// --- Chat log ---
function addChat(role, text) {
  const log = document.getElementById('chatLog');
  const div = document.createElement('div');
  div.className = 'chat-msg';
  const name = role === 'user' ? 'You' : 'Ponyu';
  div.innerHTML = '<span class="' + role + '">' + name + ':</span> ' + text;
  log.appendChild(div);
  log.scrollTop = log.scrollHeight;
}

// --- Button commands ---
async function cmd(command) {
  const status = document.getElementById('status');
  status.textContent = '⏳ ' + command + '...';
  try {
    const r = await fetch('/cmd?' + command);
    const data = await r.json();
    if (data.ok) {
      const msg = data.response || data.message || command;
      status.textContent = '✅ ' + msg;
      addChat('ponyu', msg);
      speak(msg);
      if (data.battery !== undefined) updateBattery(data.battery);
    } else {
      status.textContent = '❌ ' + (data.error || 'Error');
    }
  } catch(e) { status.textContent = '❌ Connection error'; }
}

// --- Stop everything ---
async function stopAll() {
  document.getElementById('status').textContent = '⏹️ Stopping...';
  try {
    await fetch('/autonomy/stop');
    await fetch('/cmd?stop');
    document.getElementById('status').textContent = '⏹️ Stopped. Ponyu is standing.';
    addChat('ponyu', 'Okay, I stopped!');
    speak('Okay, I stopped!');
  } catch(e) { document.getElementById('status').textContent = '❌ Connection error'; }
}

// --- Chat (text or voice) ---
async function sendChat() {
  const input = document.getElementById('chatInput');
  const text = input.value.trim();
  if (!text) return;
  input.value = '';
  addChat('user', text);
  document.getElementById('status').textContent = '🤔 Thinking...';
  try {
    const r = await fetch('/chat', {
      method: 'POST',
      headers: {'Content-Type': 'application/json'},
      body: JSON.stringify({text: text})
    });
    const data = await r.json();
    if (data.ok) {
      const msg = data.response || 'Done!';
      addChat('ponyu', msg);
      speak(msg);
      document.getElementById('status').textContent = '✅ ' + msg;
      if (data.battery !== undefined) updateBattery(data.battery);
    } else {
      const errMsg = data.error || 'Unknown error';
      addChat('ponyu', 'Oops! ' + errMsg);
      document.getElementById('status').textContent = '❌ ' + errMsg;
    }
  } catch(e) { document.getElementById('status').textContent = '❌ Connection error'; }
}

// --- Voice recognition ---
function toggleVoice() {
  const btn = document.getElementById('micBtn');
  if (isRecording) { if (recognition) recognition.stop(); return; }
  const SR = window.SpeechRecognition || window.webkitSpeechRecognition;
  if (!SR) { alert('Voice not supported. Try Chrome or Safari.'); return; }
  recognition = new SR();
  recognition.lang = 'en-US';
  recognition.interim = false;
  recognition.maxAlternatives = 1;
  recognition.onstart = () => {
    isRecording = true; btn.classList.add('recording'); btn.textContent = '⏹';
    document.getElementById('status').textContent = '🎤 Listening...';
  };
  recognition.onresult = (event) => {
    const transcript = event.results[0][0].transcript;
    document.getElementById('chatInput').value = transcript;
    addChat('user', transcript + ' 🎤');
    sendChat();
  };
  recognition.onerror = (event) => {
    document.getElementById('status').textContent = '🎤 Voice error: ' + event.error;
  };
  recognition.onend = () => {
    isRecording = false; btn.classList.remove('recording'); btn.textContent = '🎤';
  };
  recognition.start();
}

function updateBattery(pct) {
  const fill = document.getElementById('battery-fill');
  fill.style.width = pct + '%';
  fill.style.background = pct > 50 ? '#4CAF50' : pct > 20 ? '#FF9800' : '#f44336';
}

// --- Camera feed ---
function checkCamera() {
  const img = document.getElementById('cameraFeed');
  img.onload = () => { img.style.display = 'block'; };
  img.onerror = () => { img.style.display = 'none'; };
  img.src = '/video?' + Date.now();
}

window.addEventListener('load', () => {
  loadTTS();
  fetch('/cmd?hello').then(r => r.json()).then(data => {
    if (data.ok && data.message) { addChat('ponyu', data.message); speak(data.message); }
    if (data.battery !== undefined) updateBattery(data.battery);
  }).catch(() => {});
  if (window.speechSynthesis) window.speechSynthesis.getVoices();
  setTimeout(checkCamera, 2000);
});
</script>
</body>
</html>"""


class PonyuHandler(BaseHTTPRequestHandler):
    controller: PonyuController = None  # set before server starts

    def do_GET(self):
        parsed = urlparse(self.path)
        path = parsed.path
        qs = parse_qs(parsed.query)

        if path == "/" or path == "/index.html":
            self.send_response(200)
            self.send_header("Content-Type", "text/html")
            self.end_headers()
            self.wfile.write(HTML_PAGE.encode())
            return

        if path == "/cmd":
            # parse_qs ignores keys without values (e.g. ?walk), so check raw query first
            raw_q = parsed.query.split("&")[0] if parsed.query else ""
            if "=" in raw_q:
                command = qs.get("command", [raw_q.split("=")[0]])[0]
            else:
                command = raw_q if raw_q else ""

            if command and command not in COMMANDS:
                self._json({"ok": False, "error": f"Unknown: {command}"})
                return

            if not command:
                self._json({"ok": False, "error": "No command"})
                return

            result = self.controller.execute(command)
            self._json(result)
            return

        if path == "/status":
            self._json({
                "busy": self.controller.is_busy(),
                "battery": int(self.controller.robot.read_battery()),
                "vision": self.controller.get_vision_status(),
            })
            return

        if path == "/video":
            self._serve_video()
            return

        if path == "/vision":
            self._json(self.controller.get_vision_status())
            return

        if path == "/autonomy/stop":
            result = self.controller.stop_autonomy()
            self._json(result)
            return

        self.send_response(404)
        self.end_headers()

    def do_POST(self):
        parsed = urlparse(self.path)
        path = parsed.path

        if path == "/chat":
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length)
            try:
                data = json.loads(body)
                text = data.get("text", "")
            except Exception:
                self._json({"ok": False, "error": "Invalid JSON"})
                return

            if not text.strip():
                self._json({"ok": False, "error": "Empty text"})
                return

            print(f"[Ponyou] Chat: {text}")
            result = self.controller.chat(text)
            print(f"[Ponyou] Response: {result.get('response', '')}")
            self._json(result)
            return

        self.send_response(404)
        self.end_headers()

    def _serve_video(self):
        """Stream MJPEG frames from the camera."""
        if not self.controller.vision or not self.controller.vision.is_running():
            self.send_response(503)
            self.send_header("Content-Type", "text/plain")
            self.end_headers()
            self.wfile.write(b"Camera not available")
            return

        self.send_response(200)
        self.send_header("Age", "0")
        self.send_header("Cache-Control", "no-cache, private")
        self.send_header("Pragma", "no-cache")
        self.send_header("Content-Type", "multipart/x-mixed-replace; boundary=FRAME")
        self.end_headers()

        try:
            while self.controller.vision and self.controller.vision.is_running():
                jpeg = self.controller.vision.get_frame_jpeg(quality=40)
                if jpeg is None:
                    time.sleep(0.1)
                    continue
                self.wfile.write(b"--FRAME\r\n")
                self.send_header("Content-Type", "image/jpeg")
                self.send_header("Content-Length", str(len(jpeg)))
                self.end_headers()
                self.wfile.write(jpeg)
                self.wfile.write(b"\r\n")
        except Exception:
            pass

    def _json(self, data):
        self.send_response(200)
        self.send_header("Content-Type", "application/json")
        self.end_headers()
        self.wfile.write(json.dumps(data).encode())

    def log_message(self, format, *args):
        pass  # suppress logs


# ---------------------------------------------------------------------------
# CLI mode
# ---------------------------------------------------------------------------

def cli_main(args):
    offsets = load_offsets(args.offsets) if os.path.exists(args.offsets) else None
    robot = XGORobot(offsets=offsets, noise=True)
    controller = PonyuController(robot, args.offsets)

    command = args.cli
    if command not in COMMANDS:
        print(f"Available commands: {', '.join(COMMANDS.keys())}")
        sys.exit(1)

    result = controller.execute(command, duration=args.duration)
    if result.get("ok"):
        print(f"✅ {result.get('message', 'Done')}")
    else:
        print(f"❌ {result.get('error', 'Error')}")
        sys.exit(1)


def chat_main(args):
    offsets = load_offsets(args.offsets) if os.path.exists(args.offsets) else None
    robot = XGORobot(offsets=offsets, noise=True)
    controller = PonyuController(robot, args.offsets)

    text = args.chat
    print(f"You: {text}")
    result = controller.chat(text)
    if result.get("ok"):
        print(f"Ponyu: {result.get('response', 'Done')}")
        if result.get("command"):
            print(f"  (executed: {result['command']})")
    else:
        print(f"❌ {result.get('error', 'Error')}")
        sys.exit(1)


# ---------------------------------------------------------------------------
# Web server mode
# ---------------------------------------------------------------------------

def web_main(args):
    offsets = load_offsets(args.offsets) if os.path.exists(args.offsets) else None
    robot = XGORobot(offsets=offsets, noise=True)
    controller = PonyuController(robot, args.offsets)

    PonyuHandler.controller = controller

    port = args.port
    server = ThreadingHTTPServer(("0.0.0.0", port), PonyuHandler)
    print(f"\n[Ponyou] Web interface on http://0.0.0.0:{port}")
    print(f"[Ponyou] On your phone: http://10.42.0.1:{port}")
    print("[Ponyou] Press Ctrl+C to stop\n")

    def signal_handler(sig, frame):
        print("\n[Ponyou] Shutting down...")
        controller.cleanup()
        robot.reset_pose()
        server.shutdown()
        sys.exit(0)

    signal.signal(signal.SIGINT, signal_handler)

    try:
        server.serve_forever()
    except KeyboardInterrupt:
        controller.cleanup()
        robot.reset_pose()
        server.shutdown()


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(description="Ponyou — JunoClaw robot agent")
    parser.add_argument("--offsets", default="/home/pi/xgo_offsets.json",
                        help="Path to saved offsets")
    parser.add_argument("--port", type=int, default=8080,
                        help="Web server port (default 8080)")
    parser.add_argument("--cli", default=None,
                        help="Run single command and exit (stand, sit, walk, stop, etc.)")
    parser.add_argument("--chat", default=None,
                        help="Chat with Ponyu in natural language (e.g. 'hey ponyu walk forward')")
    parser.add_argument("--duration", type=float, default=4.0,
                        help="Duration for walk commands (seconds)")
    args = parser.parse_args()

    if args.cli:
        cli_main(args)
    elif args.chat:
        chat_main(args)
    else:
        web_main(args)


if __name__ == "__main__":
    main()
