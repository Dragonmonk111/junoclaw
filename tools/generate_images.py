#!/usr/bin/env python3
"""Generate all article images via Flux.1-schnell on Akash.

Usage:
    python generate_images.py --flux-url http://<akash-provider-uri> --output ./article-images
    python generate_images.py --flux-url http://localhost:8000 --output ./article-images

Flux.1-schnell API (diffusers FastAPI):
    POST /generate
    Body: {"prompt": "...", "num_inference_steps": 4, "guidance_scale": 0.0}
    Returns: image/png

For the cro7nis/flux-schnell-on-akash deployment, the API is:
    POST /text-to-image
    Body: {"prompt": "..."}
    Returns: image/png
"""

import argparse
import json
import os
import sys
import time
import urllib.request
import urllib.error
from pathlib import Path

# ─── All prompts from both articles ───

PROMPTS = [
    # Scaling Ages article (8 prompts)
    {
        "id": "scaling_01_cover",
        "article": "ROBOT_SCALING_AGES",
        "label": "Cover — Humanoid robot in desert",
        "prompt": "A lone humanoid robot standing at the edge of a vast desert at dawn, its chest cavity open revealing a glowing geometric proof — a small crystalline 128-byte shard emitting blue light, behind the robot a trail of footprints each containing a tiny glowing Merkle tree root, the desert stretches to a horizon where a massive blockchain consensus tower rises like a mirage, in the style of a 2D hand-drawn manga cross-section meets ukiyo-e woodblock print, warm sepia and sand tones with electric blue accents for the proof shard, wide cinematic composition, wabi-sabi imperfection in every line",
    },
    {
        "id": "scaling_01b_unitree",
        "article": "ROBOT_SCALING_AGES",
        "label": "Unitree Debut — Shanghai stock exchange",
        "prompt": "A massive Shanghai stock exchange building made of brass and glass, its digital ticker showing a humanoid robot company logo and a 600% upward green arrow, tiny figures of investors in suits looking up at the ticker, a humanoid robot standing on a pedestal in front of the building holding a small glowing 128-byte proof shard in one hand, the scene is split between the chaotic market floor below and the calm blue light of the proof shard above, in the style of Japanese financial newspaper illustration meets cyberpunk manga, sepia and black ink with green and electric blue accents, dramatic perspective",
    },
    {
        "id": "scaling_02_circuits",
        "article": "ROBOT_SCALING_AGES",
        "label": "The Five Circuits — Crystalline pagoda",
        "prompt": "An exploded isometric diagram of five nested crystalline circuits arranged in ascending order like a Japanese pagoda, each floor glowing with a different color — bottom floor amber with sensor waveform patterns, second floor green with zone boundary lines, third floor deep blue with validator key icons, fourth floor violet with interlocking hash chains connecting all floors, a tiny 128-byte shard floats above the top like a star, in the style of architectural blueprint meets manga technical illustration, ink outlines with watercolor fills, each floor labeled with tiny kanji-like symbols representing its function, dark background with the circuits glowing",
    },
    {
        "id": "scaling_03_benchmark",
        "article": "ROBOT_SCALING_AGES",
        "label": "The Benchmark — Japanese brass stopwatch",
        "prompt": "A traditional Japanese stopwatch made of brass and crystal, its face divided into five concentric rings each measuring a different duration, the outer ring shows 80ms in amber, the next 119ms in green, then 51ms in blue, then 68ms in violet, the center shows 187ms in white with a small 128-byte crystal shard at the exact center, the stopwatch sits on a wooden workbench surrounded by scattered circuit diagrams and empty tea cups, in the style of 19th century scientific instrument illustration meets manga panel design, warm sepia with the crystal shard glowing electric blue",
    },
    {
        "id": "scaling_04_ages",
        "article": "ROBOT_SCALING_AGES",
        "label": "The Five Ages — Vertical landscape",
        "prompt": "A sweeping panoramic landscape divided into five horizontal bands representing five ages of robotics hardware, bottom band shows a single small robot in a workshop with hand tools, second band shows a fleet of robots in a warehouse with basic GPUs visible, third band shows a futuristic factory with GPU racks alongside robots, fourth band shows a vast city with thousands of robots and data centers glowing, top band shows an abstract cosmic-scale network of robots covering a planet surface with ASIC chips embedded in the ground, each band transitions smoothly into the next like a geological stratum, in the style of Hokusai's Great Wave but vertical showing time ascending, sepia at bottom transitioning to electric blue at top, hand-drawn manga linework throughout",
    },
    {
        "id": "scaling_05_proof_size",
        "article": "ROBOT_SCALING_AGES",
        "label": "The Proof Size — Shard between chopsticks",
        "prompt": "A tiny crystalline shard no larger than a rice grain held between chopsticks above a traditional Japanese tea ceremony table, the shard emits a soft blue glow containing mathematical symbols and elliptic curve points, on the table below are scattered large scrolls of sensor data and rosbag files representing the full robot telemetry, a magnifying glass reveals that the shard contains exactly three G1 points and three Fq scalars, in the style of Japanese still life illustration meets technical diagram, warm cream and sand colors with the shard glowing electric blue, extreme close-up composition with shallow depth of field",
    },
    {
        "id": "scaling_06_trust_spectrum",
        "article": "ROBOT_SCALING_AGES",
        "label": "The Trust Spectrum — TEE to pure crypto",
        "prompt": "A horizontal spectrum diagram drawn in ink on rice paper, left side labeled TEE in warm amber showing a hardware lockbox with a glowing seal, right side labeled PURE CRYPTO in cool blue showing mathematical equations floating freely, five stepping stones cross the spectrum from left to right each labeled with a different approach, at the far right a small figure reaches the shore of pure cryptography, in the style of Japanese ink painting meets mathematical diagram, minimal color with amber on left transitioning to blue on right, contemplative mood",
    },
    {
        "id": "scaling_07_epilogue",
        "article": "ROBOT_SCALING_AGES",
        "label": "Epilogue — Robot in zen garden",
        "prompt": "A quiet scene at dusk: a single robot sitting motionless in a zen garden, its chest panel closed but a faint blue glow visible through the seams showing the proof shard inside, before the robot a trail of Merkle tree roots fades into the raked sand like footprints being washed by rain, in the distance a consensus tower glows softly on the horizon, cherry blossoms drift through the scene carrying tiny 128-byte shards, the robot is still but not off, in the style of minimalist Japanese ink painting with maximum negative space, black ink on cream paper with only electric blue for the proof glow and pale pink for blossoms, wabi-sabi imperfection, the feeling of quiet trust",
    },
    # Melange article (5 prompts)
    {
        "id": "melange_01_cover",
        "article": "JUNOCLAW_FULL_STACK_MELANGE",
        "label": "Cover — The Six Layers hexagonal tower",
        "prompt": "A vast hexagonal tower rising from a desert floor at golden hour, six distinct horizontal bands of light glowing within its crystalline structure — bottom band warm amber with tiny glowing contract glyphs, second band green with interconnected node meshes, third band deep blue with waveform patterns, fourth band violet with elliptic curve symbols, fifth band indigo with lattice cryptography patterns, top band bright electric blue with a humanoid robot silhouette operating machinery, at the very peak a small 128-byte crystalline shard emits a beam of light into the sky, in the style of Japanese woodblock print meets architectural cross-section diagram, warm sepia base transitioning to cool blues at the top, hand-drawn linework with watercolor fills, wide cinematic composition",
    },
    {
        "id": "melange_02_contracts",
        "article": "JUNOCLAW_FULL_STACK_MELANGE",
        "label": "The Four Contracts — Stone tablets in shrine",
        "prompt": "Four glowing stone tablets arranged in a semicircle inside a traditional Japanese shrine, each tablet inscribed with different glowing symbols — the first showing a registry of names with light connections branching outward, the second showing a crystal lens refracting proof-light into rainbow bands, the third showing a soulbound thread weaving through a key, the fourth showing coral-like memory structures recording pulses, small mechanical sprites tend to each tablet, the shrine is made of dark wood with paper lanterns providing warm amber light, in the style of ukiyo-e woodblock print meets technical illustration, sepia and amber tones with electric blue accents for the glowing symbols, contemplative and sacred mood",
    },
    {
        "id": "melange_03_ledger",
        "article": "JUNOCLAW_FULL_STACK_MELANGE",
        "label": "The Contract Ledger — Horizontal scroll",
        "prompt": "A long horizontal scroll made of aged rice paper unrolled across a wooden workbench, ten distinct contract seals stamped along its length in two rows, the top four seals glow with steady amber light indicating mainnet deployment, the bottom six seals glow with softer green light indicating testnet or ready status, each seal contains a tiny pictogram representing its function, an ink brush moves across the scroll writing gas measurements next to each seal, in the style of Japanese calligraphy illustration meets infographic, warm cream and amber tones with green and blue accents for the seals",
    },
    {
        "id": "melange_04_delivery",
        "article": "JUNOCLAW_FULL_STACK_MELANGE",
        "label": "The Robotics Delivery Stack — Isometric warehouse",
        "prompt": "An exploded isometric diagram of a warehouse robot deployment, at the center a humanoid robot on a factory floor surrounded by sensor halos, to the left a Python bridge represented as a traditional Japanese wooden bridge with data streams flowing across it like water, to the right a Rust prover daemon represented as a small forge hammering crystalline proof shards, below them a Docker container represented as a lacquered bento box containing the full stack neatly organized, above the robot a fleet dashboard represented as a paper lantern showing green and red status indicators, in the background a chain of mountains representing the Juno blockchain with blocks stacked like stone pagodas, in the style of technical illustration meets Japanese landscape painting, warm industrial tones with electric blue for proof shards and green for status indicators, clean isometric perspective",
    },
    {
        "id": "melange_05_built",
        "article": "JUNOCLAW_FULL_STACK_MELANGE",
        "label": "The Built Stack — Japanese workshop at dawn",
        "prompt": "A traditional Japanese workshop at dawn with every tool hung neatly on the wall in its proper place, the walls are covered with completed work — five framed circuit diagrams glowing with blue light, a shelf holding four mainnet contract tablets lit with amber, a forge with proof shards cooling on the anvil, a wooden bridge model with data streams, a bento box with the full deployment stack, a paper lantern fleet dashboard glowing green, a key cabinet with organized robot keys, a compliance scroll with ISO stamps, a brass cost calculator, and a tiny crystalline soak-test hourglass still running with sand flowing, through the window the Juno blockchain mountains are visible with blocks stacking in real time, in the style of Japanese workshop illustration meets technical diagram, warm amber and wood tones with electric blue accents for the technology elements, everything in its place, nothing unfinished, the feeling of quiet competence",
    },
]


def generate_image(flux_url: str, prompt: str, output_path: Path, timeout: int = 60) -> bool:
    """Send a prompt to the Flux API and save the resulting image."""
    # Try the text-to-image endpoint (cro7nis/flux-schnell-on-akash format)
    payload = json.dumps({"prompt": prompt}).encode("utf-8")

    endpoints = ["/text-to-image", "/generate", "/predict"]

    for endpoint in endpoints:
        url = f"{flux_url.rstrip('/')}{endpoint}"
        try:
            req = urllib.request.Request(
                url,
                data=payload,
                headers={"Content-Type": "application/json"},
                method="POST",
            )
            print(f"  POST {url} ...")
            with urllib.request.urlopen(req, timeout=timeout) as resp:
                content_type = resp.headers.get("Content-Type", "")

                if "image" in content_type:
                    # Direct image response
                    image_data = resp.read()
                    output_path.write_bytes(image_data)
                    print(f"  ✓ Saved: {output_path.name} ({len(image_data)} bytes)")
                    return True
                elif "application/json" in content_type:
                    # JSON response with base64 image or URL
                    data = json.loads(resp.read())
                    if "image" in data:
                        import base64
                        image_data = base64.b64decode(data["image"])
                        output_path.write_bytes(image_data)
                        print(f"  ✓ Saved: {output_path.name} ({len(image_data)} bytes)")
                        return True
                    elif "url" in data:
                        img_url = data["url"]
                        print(f"  Fetching image from: {img_url}")
                        with urllib.request.urlopen(img_url, timeout=timeout) as img_resp:
                            image_data = img_resp.read()
                            output_path.write_bytes(image_data)
                            print(f"  ✓ Saved: {output_path.name} ({len(image_data)} bytes)")
                            return True
                    else:
                        print(f"  Unexpected JSON keys: {list(data.keys())}")
                        continue
                else:
                    # Maybe it's an image despite content-type
                    data = resp.read()
                    if len(data) > 1000:  # likely an image
                        output_path.write_bytes(data)
                        print(f"  ✓ Saved: {output_path.name} ({len(data)} bytes)")
                        return True
                    print(f"  Unexpected content-type: {content_type}")
                    continue

        except urllib.error.HTTPError as e:
            print(f"  ✗ {endpoint}: HTTP {e.code} - {e.reason}")
            continue
        except urllib.error.URLError as e:
            print(f"  ✗ {endpoint}: {e.reason}")
            continue
        except Exception as e:
            print(f"  ✗ {endpoint}: {e}")
            continue

    return False


def main():
    parser = argparse.ArgumentParser(
        description="Generate all article images via Flux.1-schnell on Akash"
    )
    parser.add_argument(
        "--flux-url",
        required=True,
        help="Flux API URL (e.g., http://provider-uri:8000)",
    )
    parser.add_argument(
        "--output",
        default="./article-images",
        help="Output directory for images (default: ./article-images)",
    )
    parser.add_argument(
        "--timeout",
        type=int,
        default=120,
        help="Timeout per image in seconds (default: 120)",
    )
    parser.add_argument(
        "--only",
        help="Only generate images matching this filter (e.g., 'scaling' or 'melange')",
    )
    parser.add_argument(
        "--list",
        action="store_true",
        help="List all prompts and exit",
    )

    args = parser.parse_args()

    if args.list:
        print(f"Total prompts: {len(PROMPTS)}\n")
        for i, p in enumerate(PROMPTS, 1):
            print(f"{i:2d}. [{p['article']}] {p['id']}: {p['label']}")
        return

    output_dir = Path(args.output)
    output_dir.mkdir(parents=True, exist_ok=True)

    prompts = PROMPTS
    if args.only:
        prompts = [p for p in PROMPTS if args.only.lower() in p["article"].lower()]
        print(f"Filtered to {len(prompts)} prompts matching '{args.only}'")

    print(f"\nGenerating {len(prompts)} images via Flux at {args.flux_url}")
    print(f"Output: {output_dir.resolve()}\n")

    succeeded = 0
    failed = 0
    start_time = time.time()

    for i, p in enumerate(prompts, 1):
        print(f"\n[{i}/{len(prompts)}] {p['label']}")

        output_path = output_dir / f"{p['id']}.png"

        if output_path.exists():
            print(f"  Already exists, skipping: {output_path.name}")
            succeeded += 1
            continue

        ok = generate_image(args.flux_url, p["prompt"], output_path, timeout=args.timeout)
        if ok:
            succeeded += 1
        else:
            failed += 1
            # Save the prompt for retry
            prompt_file = output_dir / f"{p['id']}.txt"
            prompt_file.write_text(p["prompt"])
            print(f"  Prompt saved to: {prompt_file.name} (retry manually)")

    elapsed = time.time() - start_time
    print(f"\n{'='*60}")
    print(f"Done in {elapsed:.1f}s")
    print(f"  ✓ Succeeded: {succeeded}")
    print(f"  ✗ Failed:    {failed}")
    print(f"  Output:      {output_dir.resolve()}")

    if failed > 0:
        print(f"\nFailed prompts saved as .txt files for manual retry.")
        print(f"Or check your Flux deployment is running at: {args.flux_url}")


if __name__ == "__main__":
    main()
