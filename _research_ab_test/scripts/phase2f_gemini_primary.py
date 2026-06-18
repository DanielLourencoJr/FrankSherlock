#!/usr/bin/env python3
"""Phase 2f: Primary classification using Google Gemini vision models.

Uses the exact PRIMARY_PROMPT from sherlock/desktop/src-tauri/src/classify.rs.
Run with: python phase2f_gemini_primary.py --input C:/path/to/images
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

PRIMARY_PROMPT = (
    'Classify this image. Return ONLY valid JSON with these fields:\n'
    '{"media_type":"(anime|manga|photo|screenshot|document|artwork|other)",'
    '"contains_text":true|false,'
    '"description":"specific description under 12 words",'
    '"confidence":0.0}\n'
    'Rules: '
    'media_type is the image category. '
    'contains_text=true if any visible text (UI, receipt, subtitles, signs). '
    'description: be specific, not generic \u2014 name the subject, setting, or style. '
    'confidence: 0.0-1.0 how sure you are.'
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
        "generationConfig": {"temperature": 0.1, "maxOutputTokens": 512},
    }
    max_retries = 10
    for attempt in range(max_retries):
        payload_str = json.dumps(payload)
        print(f"    [API call to {model}, payload {len(payload_str)} bytes]")
        try:
            resp = requests.post(
                url,
                headers={"Content-Type": "application/json"},
                data=payload_str,
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
            print(f"    [API error {resp.status_code}: {resp.text[:200]}]")
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
        description="Phase 2f: Gemini vision PRIMARY_PROMPT benchmark"
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
    print("Phase 2f: Gemini Vision — Primary Classification (PRIMARY_PROMPT)")
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
                model, api_key, images, PRIMARY_PROMPT, i, repeats, limits,
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
        "phase": "2f_gemini_primary",
        "prompt": "PRIMARY_PROMPT",
        "input_dir": str(input_dir),
        "total_images": len(images),
        "models_tested": requested_models,
        "repeats_per_model": min(args.repeats, max(
            (DEFAULT_GEMINI_LIMITS.get(m, {}).get("rpd", 1500) // len(images)) if len(images) > 0 else args.repeats
            for m in requested_models
        )),
        "total_api_calls": len(all_entries),
        "errors": sum(1 for e in all_entries if e["error"]),
        "results": all_entries,
    }

    output_path = OUTPUT_DIR / "gemini_primary_report.json"
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
