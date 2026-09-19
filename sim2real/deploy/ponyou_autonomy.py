"""Ponyou Autonomy — explore mode, person following, return-to-base, auto-stop.

Uses the walk policy for locomotion and PonyuVision for perception.
Runs as background threads that command the robot's walk policy with
different speed/heading values based on what the camera sees.

Modes:
    explore   — walk forward, avoid obstacles, turn at walls
    follow    — follow the nearest person
    come_home — walk back toward the operator (reverse heading)
    patrol    — walk in a square pattern (forward, turn, repeat)

Usage:
    from ponyou_autonomy import PonyuAutonomy
    autonomy = PonyuAutonomy(controller, vision)
    autonomy.start_mode("explore")
    # ... robot walks around avoiding obstacles ...
    autonomy.stop()  # returns to standing
"""

from __future__ import annotations

import threading
import time
import math
import random


class PonyuAutonomy:
    """Autonomous behavior modes for Ponyu.

    Wraps the PonyuController and PonyuVision to provide autonomous
    behaviors. Each mode runs in a background thread.
    """

    def __init__(self, controller, vision=None):
        self.controller = controller
        self.vision = vision
        self._thread = None
        self._running = False
        self._mode = None
        self._lock = threading.Lock()

    def start_mode(self, mode: str, duration: float = 60.0):
        """Start an autonomous mode. Stops any current mode first."""
        self.stop()
        valid = {"explore", "follow", "come_home", "patrol"}
        if mode not in valid:
            return {"ok": False, "error": f"Unknown mode: {mode}. Valid: {valid}"}

        with self._lock:
            self._mode = mode
            self._running = True

        self._thread = threading.Thread(
            target=self._run_mode, args=(mode, duration), daemon=True
        )
        self._thread.start()
        return {"ok": True, "mode": mode, "duration": duration}

    def stop(self):
        """Stop autonomous mode and return to standing."""
        self._running = False
        if self._thread:
            self._thread.join(timeout=3.0)
        self._thread = None
        self._mode = None

        # Ensure robot is standing
        try:
            self.controller.robot.reset_pose()
        except Exception:
            pass

    def is_running(self) -> bool:
        return self._running

    def get_mode(self) -> str | None:
        return self._mode

    def _run_mode(self, mode: str, duration: float):
        """Main autonomy loop — runs in background thread."""
        print(f"[Autonomy] Starting mode: {mode} for {duration}s")
        start_time = time.time()

        try:
            if mode == "explore":
                self._explore(duration, start_time)
            elif mode == "follow":
                self._follow(duration, start_time)
            elif mode == "come_home":
                self._come_home(duration, start_time)
            elif mode == "patrol":
                self._patrol(duration, start_time)
        except Exception as e:
            print(f"[Autonomy] Error: {e}")
        finally:
            print("[Autonomy] Mode ended, returning to stand.")
            self._running = False
            try:
                self.controller.robot.reset_pose()
            except Exception:
                pass

    def _walk(self, speed: float, heading: float, steps: int):
        """Walk with given speed/heading for N steps. Returns if interrupted."""
        if not self._running:
            return False

        # Use the controller's walk policy directly
        result = self.controller.execute("walk", duration=steps / 50.0)
        return result.get("ok", False)

    def _turn(self, direction: str, duration: float = 1.5):
        """Turn left or right."""
        if not self._running:
            return
        self.controller.execute(direction, duration=duration)

    def _check_obstacle(self) -> dict | None:
        """Check vision for obstacles. Returns obstacle info or None."""
        if not self.vision or not self.vision.is_running():
            return None
        return self.vision.get_obstacle_info()

    def _check_person(self) -> dict | None:
        """Check vision for person. Returns person info or None."""
        if not self.vision or not self.vision.is_running():
            return None
        return self.vision.get_person_info()

    def _explore(self, duration: float, start_time: float):
        """Explore mode: walk forward, avoid obstacles, turn at walls."""
        print("[Autonomy] Explore: walking forward, avoiding obstacles")
        walk_duration = 3.0  # walk 3 seconds at a time

        while self._running and (time.time() - start_time) < duration:
            obs = self._check_obstacle()

            if obs and obs.get("blocked"):
                direction = obs.get("direction", "center")
                print(f"[Autonomy] Obstacle {direction}, dist={obs.get('distance')}")

                if direction == "left":
                    print("[Autonomy] Turning right to avoid")
                    self._turn("turn-right", 1.5)
                elif direction == "right":
                    print("[Autonomy] Turning left to avoid")
                    self._turn("turn-left", 1.5)
                else:
                    # Center blocked — pick a random direction
                    turn = random.choice(["turn-left", "turn-right"])
                    print(f"[Autonomy] Center blocked, {turn}")
                    self._turn(turn, 2.0)
            else:
                # Clear path — walk forward
                print("[Autonomy] Path clear, walking forward")
                self._walk(speed=0.08, heading=0.0, steps=150)

            time.sleep(0.1)

    def _follow(self, duration: float, start_time: float):
        """Follow mode: follow the nearest person."""
        print("[Autonomy] Follow: looking for people to follow")

        while self._running and (time.time() - start_time) < duration:
            person = self._check_person()

            if person and person.get("detected"):
                cx = person.get("cx", 0.5)  # 0=left, 1=right
                area = person.get("area", 0)

                # Steer toward person
                error = cx - 0.5  # -0.5 (person left) to +0.5 (person right)
                heading = error * 0.6  # scale to heading command

                # If person is close enough (large area), stop and wait
                if area > 15000:
                    print(f"[Autonomy] Person close (area={area}), waiting")
                    self.controller.execute("stop")
                    time.sleep(1.0)
                else:
                    print(f"[Autonomy] Following person at cx={cx:.2f}, heading={heading:.2f}")
                    # Walk toward person
                    if abs(error) > 0.15:
                        # Person is off-center — turn toward them
                        if error > 0:
                            self.controller.execute("turn-right", 1.0)
                        else:
                            self.controller.execute("turn-left", 1.0)
                    else:
                        # Person is roughly centered — walk forward
                        self._walk(speed=0.06, heading=0.0, steps=100)
            else:
                # No person detected — turn to look around
                print("[Autonomy] No person found, turning to look")
                self._turn("turn-left", 1.0)

            time.sleep(0.2)

    def _come_home(self, duration: float, start_time: float):
        """Come home: walk in a spiral pattern to return to general area."""
        print("[Autonomy] Come home: spiraling back")
        spiral_radius = 1.0

        while self._running and (time.time() - start_time) < duration:
            # Walk forward, then turn slightly, increasing turn angle each time
            self._walk(speed=0.08, heading=0.0, steps=100)

            # Check for obstacles
            obs = self._check_obstacle()
            if obs and obs.get("blocked"):
                self._turn("turn-right", 1.5)

            # Gradually increase turn to spiral
            turn_duration = min(3.0, 0.5 + (time.time() - start_time) * 0.05)
            self._turn("turn-right", turn_duration)

            time.sleep(0.1)

    def _patrol(self, duration: float, start_time: float):
        """Patrol mode: walk in a square pattern."""
        print("[Autonomy] Patrol: walking in a square")
        side_duration = 4.0  # walk 4 seconds per side

        while self._running and (time.time() - start_time) < duration:
            for side in range(4):
                if not self._running or (time.time() - start_time) >= duration:
                    break

                # Check for obstacles before walking
                obs = self._check_obstacle()
                if obs and obs.get("blocked"):
                    print(f"[Autonomy] Patrol: obstacle at {obs['direction']}, avoiding")
                    self._turn("turn-right", 1.5)

                # Walk forward one side
                print(f"[Autonomy] Patrol: side {side + 1}/4")
                self._walk(speed=0.08, heading=0.0, steps=int(side_duration * 50))

                if not self._running:
                    break

                # Turn 90 degrees
                self._turn("turn-right", 2.0)

            time.sleep(0.1)

    def get_status(self) -> dict:
        return {
            "running": self._running,
            "mode": self._mode,
        }
