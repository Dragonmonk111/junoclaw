/**
 * MuJoCo Simulation Tools — expose the Dogzilla sim to LLM agents via MCP
 *
 * These tools let an LLM agent (e.g. J-Lens) reason about robot physics:
 *   sim_reset  — reset the simulator to a fallen pose (tilt range)
 *   sim_step   — apply an action vector, advance physics, return observation
 *   sim_observe — read the current observation without stepping
 *   sim_eval   — evaluate a trained policy on a tilt range, return success rate
 *
 * The tools communicate with a local Python HTTP server (sim2real/sim_server.py)
 * that wraps the MuJoCo environment. This keeps the heavy Python/mujoco
 * dependency out of the Node.js MCP process.
 *
 * Architecture:
 *   MCP tool call → HTTP fetch → sim_server.py → MuJoCo env → response
 *
 * The sim_server runs on localhost:8765 by default (SIM_SERVER_PORT env var).
 */

const SIM_SERVER_URL = process.env.SIM_SERVER_URL || "http://localhost:8765";

interface SimResponse {
  ok: boolean;
  error?: string;
  [key: string]: unknown;
}

async function simFetch(path: string, body?: Record<string, unknown>): Promise<SimResponse> {
  const url = `${SIM_SERVER_URL}${path}`;
  const init: RequestInit = {
    method: body ? "POST" : "GET",
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  };
  try {
    const resp = await fetch(url, init);
    const data = (await resp.json()) as SimResponse;
    if (!resp.ok && !data.error) {
      data.error = `HTTP ${resp.status}`;
    }
    return data;
  } catch (e) {
    return {
      ok: false,
      error: `Cannot reach sim server at ${url}: ${(e as Error).message}. ` +
        "Start it with: python sim2real/sim_server.py",
    };
  }
}

export async function simReset(params: {
  tilt_degrees?: number;
  tilt_min?: number;
  tilt_max?: number;
  seed?: number;
}): Promise<SimResponse> {
  return simFetch("/reset", {
    tilt_degrees: params.tilt_degrees,
    tilt_min: params.tilt_min,
    tilt_max: params.tilt_max,
    seed: params.seed,
  });
}

export async function simStep(params: {
  action?: number[];
}): Promise<SimResponse> {
  return simFetch("/step", {
    action: params.action,
  });
}

export async function simObserve(): Promise<SimResponse> {
  return simFetch("/observe");
}

export async function simEval(params: {
  checkpoint?: string;
  tilt_min?: number;
  tilt_max?: number;
  n_episodes?: number;
}): Promise<SimResponse> {
  return simFetch("/eval", {
    checkpoint: params.checkpoint,
    tilt_min: params.tilt_min,
    tilt_max: params.tilt_max,
    n_episodes: params.n_episodes,
  });
}

export async function simInfo(): Promise<SimResponse> {
  return simFetch("/info");
}
