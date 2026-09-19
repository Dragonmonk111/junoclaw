"""Ponyou Vision — camera + obstacle detection + person following.

Runs on the Pi CM5. Uses OpenCV for camera capture and lightweight
frame-differencing for obstacle detection. Person detection uses
a simple motion/contour approach that works without heavy ML deps.

Usage:
    from ponyou_vision import PonyuVision
    vision = PonyuVision(camera_index=0)
    vision.start()

    # Check for obstacles
    obs = vision.get_obstacle_info()
    if obs["blocked"]:
        print(f"Obstacle at {obs['direction']}")

    # Check for person
    person = vision.get_person_info()
    if person["detected"]:
        print(f"Person at ({person['cx']}, {person['cy']}), area={person['area']}")

    vision.stop()
"""

from __future__ import annotations

import math
import threading
import time

import numpy as np

try:
    import cv2
except ImportError:
    cv2 = None


class PonyuVision:
    """Camera capture + obstacle/person detection for Ponyu.

    Obstacle detection: divides the frame into left/center/right regions,
    computes brightness/edge density, and flags regions with high edge
    density as potential obstacles. Works in real-time on the Pi CM5
    without any ML model.

    Person detection: uses background subtraction + contour analysis.
    A large contour in the center of the frame is classified as "person".
    For better accuracy, you can install mediapipe or a small SSD model.
    """

    def __init__(
        self,
        camera_index: int = 0,
        frame_width: int = 320,
        frame_height: int = 240,
        fps: int = 10,
        obstacle_threshold: float = 0.15,
        person_min_area: int = 2000,
    ):
        self.camera_index = camera_index
        self.frame_width = frame_width
        self.frame_height = frame_height
        self.fps = fps
        self.obstacle_threshold = obstacle_threshold
        self.person_min_area = person_min_area

        self._cap = None
        self._thread = None
        self._running = False
        self._frame = None
        self._prev_frame = None
        self._bg_subtractor = None
        self._lock = threading.Lock()
        self._obstacle_info = {"blocked": False, "direction": "none", "distance": 1.0}
        self._person_info = {"detected": False, "cx": 0.5, "cy": 0.5, "area": 0}
        self._frame_count = 0

    def start(self) -> bool:
        """Open camera and start capture thread."""
        if cv2 is None:
            print("[PonyuVision] OpenCV not installed. Vision disabled.")
            return False

        self._cap = cv2.VideoCapture(self.camera_index)
        self._cap.set(cv2.CAP_PROP_FRAME_WIDTH, self.frame_width)
        self._cap.set(cv2.CAP_PROP_FRAME_HEIGHT, self.frame_height)
        self._cap.set(cv2.CAP_PROP_FPS, self.fps)

        if not self._cap.isOpened():
            print(f"[PonyuVision] Camera {self.camera_index} not available.")
            self._cap = None
            return False

        self._bg_subtractor = cv2.createBackgroundSubtractorMOG2(
            history=50, varThreshold=25, detectShadows=False
        )

        self._running = True
        self._thread = threading.Thread(target=self._capture_loop, daemon=True)
        self._thread.start()
        print(f"[PonyuVision] Camera {self.camera_index} started ({self.frame_width}x{self.frame_height}@{self.fps}fps)")
        return True

    def stop(self):
        """Stop capture and release camera."""
        self._running = False
        if self._thread:
            self._thread.join(timeout=2.0)
        if self._cap:
            self._cap.release()
        self._cap = None
        print("[PonyuVision] Camera stopped.")

    def is_running(self) -> bool:
        return self._running and self._cap is not None

    def _capture_loop(self):
        """Background thread: capture frames and run detection."""
        interval = 1.0 / self.fps
        while self._running and self._cap and self._cap.isOpened():
            t0 = time.time()
            ret, frame = self._cap.read()
            if not ret:
                time.sleep(0.1)
                continue

            with self._lock:
                self._frame = frame
            self._frame_count += 1

            # Run detection every 3 frames to save CPU
            if self._frame_count % 3 == 0:
                self._detect_obstacles(frame)
                self._detect_person(frame)

            elapsed = time.time() - t0
            sleep_time = interval - elapsed
            if sleep_time > 0:
                time.sleep(sleep_time)

    def _detect_obstacles(self, frame):
        """Detect obstacles using edge density in left/center/right regions."""
        gray = cv2.cvtColor(frame, cv2.COLOR_BGR2GRAY)
        gray = cv2.GaussianBlur(gray, (5, 5), 0)
        edges = cv2.Canny(gray, 50, 150)

        h, w = edges.shape
        # Divide into 3 horizontal regions: left, center, right
        regions = {
            "left": edges[:, :w // 3],
            "center": edges[:, w // 3: 2 * w // 3],
            "right": edges[:, 2 * w // 3:],
        }

        densities = {}
        for name, region in regions.items():
            density = float(np.sum(region > 0)) / (region.shape[0] * region.shape[1])
            densities[name] = density

        # Also check bottom half (ground obstacles)
        bottom_edges = edges[h // 2:, :]
        bottom_density = float(np.sum(bottom_edges > 0)) / (bottom_edges.shape[0] * bottom_edges.shape[1])

        # Determine if blocked and from which direction
        blocked = any(d > self.obstacle_threshold for d in densities.values())
        if blocked:
            max_dir = max(densities, key=densities.get)
            # Estimate "distance" inversely from density (more edges = closer)
            distance = max(0.1, 1.0 - densities[max_dir])
        else:
            max_dir = "none"
            distance = 1.0

        self._obstacle_info = {
            "blocked": blocked,
            "direction": max_dir,
            "distance": round(distance, 2),
            "left_density": round(densities["left"], 3),
            "center_density": round(densities["center"], 3),
            "right_density": round(densities["right"], 3),
            "bottom_density": round(bottom_density, 3),
        }

    def _detect_person(self, frame):
        """Detect person using background subtraction + contour analysis."""
        if self._bg_subtractor is None:
            return

        fg_mask = self._bg_subtractor.apply(frame)
        # Clean up noise
        fg_mask = cv2.morphologyEx(fg_mask, cv2.MORPH_OPEN, np.ones((5, 5), np.uint8))
        fg_mask = cv2.morphologyEx(fg_mask, cv2.MORPH_CLOSE, np.ones((15, 15), np.uint8))

        contours, _ = cv2.findContours(fg_mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)

        h, w = frame.shape[:2]
        best_area = 0
        best_cx, best_cy = 0.5, 0.5

        for contour in contours:
            area = cv2.contourArea(contour)
            if area < self.person_min_area:
                continue
            # Filter by aspect ratio — person is taller than wide
            x, y, cw, ch = cv2.boundingRect(contour)
            aspect = ch / max(cw, 1)
            if aspect < 1.0:  # skip wide objects
                continue

            if area > best_area:
                best_area = area
                best_cx = (x + cw / 2) / w  # normalized 0-1
                best_cy = (y + ch / 2) / h

        self._person_info = {
            "detected": best_area > 0,
            "cx": round(best_cx, 3),
            "cy": round(best_cy, 3),
            "area": int(best_area),
        }

    def get_obstacle_info(self) -> dict:
        """Return latest obstacle detection result."""
        return self._obstacle_info.copy()

    def get_person_info(self) -> dict:
        """Return latest person detection result."""
        return self._person_info.copy()

    def get_frame_jpeg(self, quality: int = 50) -> bytes | None:
        """Get current frame as JPEG bytes (for web streaming)."""
        with self._lock:
            if self._frame is None:
                return None
            frame = self._frame.copy()

        # Draw detection overlays
        obs = self._obstacle_info
        person = self._person_info

        h, w = frame.shape[:2]

        # Draw obstacle regions
        if obs["blocked"]:
            color_map = {"left": (0, 0, 255), "center": (0, 0, 255), "right": (0, 0, 255)}
            color = color_map.get(obs["direction"], (0, 0, 255))
            cv2.rectangle(frame, (0, 0), (w, h), color, 2)
            cv2.putText(frame, f"OBSTACLE: {obs['direction']}", (10, 20),
                        cv2.FONT_HERSHEY_SIMPLEX, 0.5, color, 1)

        # Draw person bounding box
        if person["detected"]:
            cx, cy = int(person["cx"] * w), int(person["cy"] * h)
            area = person["area"]
            box_size = int(math.sqrt(area))
            cv2.rectangle(frame,
                          (cx - box_size // 2, cy - box_size // 2),
                          (cx + box_size // 2, cy + box_size // 2),
                          (0, 255, 0), 2)
            cv2.putText(frame, "PERSON", (cx - box_size // 2, cy - box_size // 2 - 5),
                        cv2.FONT_HERSHEY_SIMPLEX, 0.5, (0, 255, 0), 1)

        ret, jpeg = cv2.imencode(".jpg", frame, [cv2.IMWRITE_JPEG_QUALITY, quality])
        if ret:
            return jpeg.tobytes()
        return None

    def get_status(self) -> dict:
        """Return vision system status."""
        return {
            "running": self.is_running(),
            "frame_count": self._frame_count,
            "obstacle": self.get_obstacle_info(),
            "person": self.get_person_info(),
        }
