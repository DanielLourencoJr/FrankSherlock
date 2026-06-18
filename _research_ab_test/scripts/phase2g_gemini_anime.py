#!/usr/bin/env python3
"""Phase 2g: Anime enrichment using Google Gemini vision models.

Uses the exact ANIME_PROMPT from sherlock/desktop/src-tauri/src/classify.rs.
Run with: python phase2g_gemini_anime.py --input C:/path/to/anime/images
"""

import argparse
import base64
import json
import os
import sys
import time
from pathlib import Path

import requests

sys.path.insert(0, str(Path(__file__).parent.parent))
from lib.common import RESULTS_DIR, load_benchmark_config, save_result

GEMINI_BASE = "https://generativelanguage.googleapis.com/v1beta/models"
OUTPUT_DIR = RESULTS_DIR / "phase2_images"

IMAGE_EXTS = {".jpg", ".jpeg", ".png", ".gif", ".webp", ".bmp", ".tiff", ".tif"}

ANIME_PROMPT = (
    'Identify this anime/manga image. Return ONLY valid JSON with schema:\n'
    '{"series":"canonical series name or null",'
    '"franchise":"franchise name or null",'
    '"characters":[{"name":"full canonical name","confidence":0.0}],'
    '"scene_summary":"short description",'
    '"confidence":0.0}\n'
    'Examples:\n'
    '{"series":"Chainsaw Man","characters":[{"name":"Denji","confidence":0.95},{"name":"Pochita","confidence":0.8}],"scene_summary":"Boy with chainsaw arms fighting a demon","confidence":0.9}\n'
    '{"series":"Youjo Senki","characters":[{"name":"Tanya Degurechaff","confidence":0.9}],"scene_summary":"Blonde girl in military uniform casting magic","confidence":0.85}\n'
    '{"series":"Shuumatsu no Valkyrie","characters":[{"name":"L\u00fc Bu","confidence":0.8}],"scene_summary":"Two warriors fighting in a divine arena","confidence":0.75}\n'
    'Now identify this image: use exact canonical names, null if unknown.'
)

DEFAULT_GEMINI_LIMITS = load_benchmark_config().get("gemini_models", {})


def mime_from_path(filepath: Path) -> str:
    ext = filepath.suffix.lower()
    return {
        ".jpg": "image/jpeg", ".jpeg": "image/jpeg",
        ".png": "image/png", ".gif": "image/gif",
        ".webp": "image/webp", ".bmp": "image/bmp",
        ".tiff": "image/tiff", ".tif": "image/tiff",
    }.get(ext, "image/png")


def call_gemini(
    model: str, api_key: str, prompt: str,
    image_b64: str, mime_type: str,
    json_mode: bool, timeout_secs: int = 120,
) -> dict:
    url = f"{GEMINI_BASE}/{model}:generateContent?key={api_key}"
    parts = [
        {"inlineData": {"mimeType": mime_type, "data": image_b64}},
        {"text": prompt},
    ]
    payload = {
        "contents": [{"parts": parts}],
        "generationConfig": {"temperature": 0.1, "maxOutputTokens": 600},
    }
    max_retries = 10
    for attempt in range(max_retries):
        try:
            resp = requests.post(
                url,
                headers={"Content-Type": "application/json"},
                data=json.dumps(payload),
                timeout=timeout_secs,
            )
        except requests.exceptions.RequestException as e:
            if attempt < max_retries - 1:
                wait = 5 * (attempt + 1)
                print(f"    [Request failed, retrying in {wait}s...]")
                time.sleep(wait)
                continue
            return {"error": str(e), "http_body": ""}

        if resp.status_code == 429:
            retry_after = 10 + (attempt * 10)
            print(f"    [429 rate limited, waiting {retry_after}s (attempt {attempt+1}/{max_retries})]")
            time.sleep(retry_after)
            continue

        if not resp.ok:
            return {"error": f"http_{resp.status_code}", "http_body": resp.text[:500]}

        data = resp.json()
        candidates = data.get("candidates", [])
        if not candidates:
            return {"error": "no_candidates", "raw_response": json.dumps(data)}
        parts = candidates[0].get("content", {}).get("parts", [])
        text = " ".join(p.get("text", "") for p in parts if "text" in p)
        usage = data.get("usageMetadata", {})
        return {
            "response": text.strip(),
            "prompt_token_count": usage.get("promptTokenCount", 0),
            "candidate_token_count": usage.get("candidatesTokenCount", 0),
        }

    return {"error": "retries_exhausted"}


def collect_images(input_dir: Path) -> list[Path]:
    images = []
    for f in input_dir.iterdir():
        if f.suffix.lower() in IMAGE_EXTS and not f.name.startswith("."):
            images.append(f)
    return sorted(images, key=lambda p: p.name)


def run_model_on_images(
    model: str, api_key: str, images: list[Path],
    prompt: str, repeat_index: int, repeats: int, limits: dict,
) -> dict:
    print(f"\n=== {model} (trial {repeat_index}/{repeats}) ===")
    rpm = limits.get("rpm", 10)
    rpd = limits.get("rpd", 1500)

    results = []
    request_times = []
    total_calls = 0

    for img_path in images:
        now = time.time()
        request_times = [t for t in request_times if t > now - 60]
        if len(request_times) >= rpm:
            sleep_secs = 60.0 - (now - request_times[0]) + 1.0
            print(f"  Rate limit: sleeping {sleep_secs:.1f}s")
            time.sleep(sleep_secs)
            request_times = [t for t in request_times if t > time.time() - 60]

        total_calls += 1
        if total_calls > rpd:
            print(f"  [WARN] Exceeded {rpd} RPD limit")

        print(f"  - {img_path.name}")
        image_b64 = base64.b64encode(img_path.read_bytes()).decode("utf-8")
        mime = mime_from_path(img_path)

        start = time.perf_counter()
        out = call_gemini(model, api_key, prompt, image_b64, mime, json_mode=False)
        elapsed = time.perf_counter() - start
        out["wall_clock_s"] = round(elapsed, 4)
        request_times.append(time.time())

        results.append({
            "file": img_path.name,
            "classify": out,
        })

    return {"model": model, "trial": repeat_index, "results": results}


def main():
    parser = argparse.ArgumentParser(
        description="Phase 2g: Gemini vision ANIME_PROMPT benchmark"
    )
    parser.add_argument(
        "--input", required=True,
        help="Path to a folder with images",
    )
    parser.add_argument(
        "--models",
        default=",".join(DEFAULT_GEMINI_LIMITS.keys()),
        help="Comma-separated Gemini models",
    )
    parser.add_argument(
        "--repeats", type=int, default=3,
        help="Number of trials per model",
    )
    parser.add_argument(
        "--api-key", default=None,
        help="Google AI API key (default: GOOGLE_API_KEY env var)",
    )
    args = parser.parse_args()

    api_key = args.api_key or os.environ.get("GOOGLE_API_KEY")
    if not api_key:
        print("[ERROR] Set GOOGLE_API_KEY env var or pass --api-key")
        sys.exit(1)

    input_dir = Path(args.input)
    if not input_dir.is_dir():
        print(f"[ERROR] Not a directory: {input_dir}")
        sys.exit(1)

    images = collect_images(input_dir)
    if not images:
        print(f"[ERROR] No image files found in {input_dir}")
        sys.exit(1)
    print(f"Found {len(images)} images in {input_dir}")

    requested_models = [m.strip() for m in args.models.split(",") if m.strip()]
    if not requested_models:
        print("[ERROR] No models specified")
        sys.exit(1)

    print("=" * 60)
    print("Phase 2g: Gemini Vision — Anime Enrichment (ANIME_PROMPT)")
    print("=" * 60)

    model_runs = []
    for model in requested_models:
        limits = DEFAULT_GEMINI_LIMITS.get(model, {"rpm": 10, "rpd": 1500})
        rpd = limits.get("rpd", 1500)
        max_repeats_safe = max(1, rpd // len(images)) if len(images) > 0 else args.repeats
        repeats = min(args.repeats, max_repeats_safe)
        if repeats < args.repeats:
            print(f"  {model}: capped to {repeats} trials (RPD={rpd}, {len(images)} images/trial)")

        for i in range(1, repeats + 1):
            run_data = run_model_on_images(
                model, api_key, images, ANIME_PROMPT, i, repeats, limits,
            )
            model_runs.append(run_data)

    all_entries = []
    for run in model_runs:
        for r in run["results"]:
            all_entries.append({
                "file": r["file"],
                "model": run["model"],
                "trial": run["trial"],
                "response": r["classify"].get("response", ""),
                "latency_s": r["classify"].get("wall_clock_s", 0),
                "error": r["classify"].get("error"),
            })

    summary = {
        "phase": "2g_gemini_anime",
        "prompt": "ANIME_PROMPT",
        "input_dir": str(input_dir),
        "total_images": len(images),
        "models_tested": requested_models,
        "total_api_calls": len(all_entries),
        "errors": sum(1 for e in all_entries if e["error"]),
        "results": all_entries,
    }

    output_path = OUTPUT_DIR / "gemini_anime_report.json"
    save_result(summary, output_path)

    print(f"\n{'=' * 60}")
    print("Summary:")
    print(f"  Images: {len(images)}")
    print(f"  API calls: {len(all_entries)}")
    print(f"  Errors: {summary['errors']}")
    print(f"\n  Results saved to: {output_path}")
    print("  Open the JSON file to inspect each model response.")
    print("=" * 60)


if __name__ == "__main__":
    main()
