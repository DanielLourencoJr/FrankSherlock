"""Replicate the exact Ollama API call Sherlock makes, stripped to essentials.
Usage: python scripts/debug_ollama.py <image_path> [model_name]
"""
import sys, json, base64, urllib.request

OLLAMA_BASE = "http://localhost:11434"

PRIMARY_PROMPT = (
    'Analyze this image and respond ONLY with valid JSON. Schema: '
    '{"media_type":"screenshot|anime|manga|photo|document|artwork|other",'
    '"contains_text":true,'
    '"is_anime_related":false,'
    '"is_document_like":false,'
    '"description":"short factual description",'
    '"series_candidates":["name"],'
    '"character_candidates":["name"],'
    '"confidence":0.0}'
    ' Rules: '
    '1) Use null-like empty arrays when unknown. '
    '2) If image has visible text (UI, receipt, scan, subtitles), contains_text=true. '
    '3) is_document_like=true for receipts/invoices/forms/scanned docs/screenshots of documents. '
    '4) series_candidates and character_candidates must be unique and max 5 items each. '
    '5) Keep description under 24 words. '
    '6) Favor precision over guesswork.'
)

SIMPLE_PROMPT = (
    'Describe this image in one sentence. '
    'Also answer: what type is it (photo, screenshot, anime, manga, document, artwork, or other)? '
    'Does it contain visible text? Is it anime-related? Is it document-like? '
    'Confidence from 0 to 1.'
)

def call(model, prompt, image_b64, json_mode):
    payload = {
        "model": model,
        "prompt": prompt,
        "stream": False,
        "options": {"temperature": 0.1, "num_predict": 500},
        "images": [image_b64],
    }
    if json_mode:
        payload["format"] = "json"

    body = json.dumps(payload).encode("utf-8")
    req = urllib.request.Request(
        f"{OLLAMA_BASE}/api/generate", data=body,
        headers={"Content-Type": "application/json"}, method="POST",
    )
    try:
        resp = urllib.request.urlopen(req, timeout=180)
        raw = resp.read().decode("utf-8")
        parsed = json.loads(raw)
        text = parsed.get("response", "")
        dur = parsed.get("total_duration", 0) / 1_000_000_000
        return text, dur
    except Exception as e:
        return None, str(e)

def main():
    if len(sys.argv) < 2:
        print("Usage: python debug_ollama.py <image_path> [model_name]")
        sys.exit(1)

    image_path = sys.argv[1]
    model = sys.argv[2] if len(sys.argv) > 2 else "qwen2.5vl:3b"

    with open(image_path, "rb") as f:
        b64 = base64.b64encode(f.read()).decode("ascii")

    print(f"Model: {model}  Image: {image_path} ({len(b64)} bytes b64)\n")

    for label, prompt, jm in [
        ("1) PRIMARY_PROMPT + format:json", PRIMARY_PROMPT, True),
        ("2) SIMPLE_PROMPT + no format",   SIMPLE_PROMPT,  False),
        ("3) SIMPLE_PROMPT + format:json", SIMPLE_PROMPT,  True),
    ]:
        text, dur = call(model, prompt, b64, jm)
        print(f"{label}")
        print(f"   Duration: {dur}s" if isinstance(dur, float) else f"   Error: {dur}")
        if text and text.strip():
            print(f"   Response ({len(text)} chars):")
            print(f"   {text[:500]}")
        else:
            print(f"   *** EMPTY ***")
        print()

if __name__ == "__main__":
    main()
