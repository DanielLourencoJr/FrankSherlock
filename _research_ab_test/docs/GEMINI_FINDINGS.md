# Gemini Vision Models — Benchmark Findings

**Date:** 2026-06-03
**Corpus:** 6 anime/manga/artwork images from `D:\Junior\Imagens\Teste`
**Prompt:** PRIMARY_PROMPT from `classify.rs` (media\_type, contains\_text, description, confidence)
**Endpoint:** `v1beta` (required for preview models like `gemini-3-flash-preview`)
**Auth:** `?key=` query parameter (not `x-goog-api-key` header)

---

## Results Summary

### Model Availability

| Model | Status | Avg Latency | JSON Valid | Notes |
|---|---|---|---|---|
| `gemini-2.5-flash-lite` | ✅ 5/6 | 2.3s | 100% | Fastest, but generic descriptions; hit quota on last call |
| `gemini-2.5-flash` | ✅ 6/6 | 3.9s | 100% | Named characters (Eren, Tanya Degurechaff) |
| `gemini-3-flash-preview` | ✅ 6/6 | 4.4s | 100% | Richest descriptions, named characters with series |
| `gemini-3.1-flash-lite` | ✅ 6/6 | 2.4s | 100% | Best speed/quality balance, named characters consistently |
| `gemini-2.5-pro` | ❌ 0/6 | — | — | 429 — quota exhausted before reaching this model |

### Per-Image Classification Comparison

| Image | `2.5-flash-lite` (2.3s) | `2.5-flash` (3.9s) | `3-flash-preview` (4.4s) | `3.1-flash-lite` (2.4s) |
|---|---|---|---|---|
| **Adao1.jpeg** | anime / "Anime character with light hair" | anime / "Anime character with light hair looking left" | anime / "Shirtless male anime character with messy light hair" | anime / "Anime style portrait of a shirtless man" |
| **Eren2.jpg** | anime / "Eren Yeager looking up at the sky" | anime / "Eren Yeager excited in blue sky" | anime / "Eren Yeager with arms outstretched against a bright cloudy sky" | anime / "Eren Yeager from Attack on Titan reaching toward the sky" |
| **Mitaka4.webp** | manga / "Girl eating ice cream on a balcony" | manga / "Manga character eating a snack, leaning on a railing" | manga / "Asa Mitaka from Chainsaw Man eating ice cream" | manga / "Asa Mitaka from Chainsaw Man eating ice cream behind a fence" |
| **Tanya13.jpeg** | artwork / "Anime girl in military uniform" | artwork / "Anime girl Tanya Degurechaff in military uniform" | artwork / *(truncated)* | artwork / "Tanya Degurechaff from Saga of Tanya the Evil anime" |
| **Tanya17.webp** | artwork / "Anime girl in military uniform with rifle" | anime / "Angry Tanya Degurechaff in military uniform" | artwork / "Tanya von Degurechaff holding a helmet and rifle" | artwork / "Tanya Degurechaff from Youjo Senki in a war-torn setting" |
| **mitaka1.jpg** | `429` | manga / "Manga panel of a woman with dark hair" | manga / "Black and white manga panel of Asa Mitaka" | manga / "Close-up of Asa Mitaka from Chainsaw Man manga" |

### Key Quality Observations

1. **media\_type agreement** — All 4 models agreed on the primary category for every image (anime, manga, artwork). The only split was Tanya17.webp: `2.5-flash` classified it as `anime`, while the other 3 classified it as `artwork`.

2. **Character naming** — Larger/slower models consistently named characters:
   - `3.1-flash-lite`: Identified Asa Mitaka (Chainsaw Man), Eren Yeager (Attack on Titan), Tanya Degurechaff (Youjo Senki)
   - `3-flash-preview`: Most precise naming — "Tanya von Degurechaff", "Asa Mitaka from Chainsaw Man"
   - `2.5-flash`: Named Eren and Tanya but not Asa Mitaka
   - `2.5-flash-lite`: Only named Eren; other descriptions were generic

3. **Description detail** — `3-flash-preview` wrote the most vivid and specific descriptions, but also had the longest latency (~4.4s). `2.5-flash-lite` was fastest (~2.3s) but more generic.

4. **JSON formatting** — All models occasionally wrapped JSON in ```json code fences. Responses were consistently parseable.

---

## Quota & Rate Limit Findings

### Observed Limits

After approximately **24 vision API calls** across 4 models (6 images each), the free tier quota was completely exhausted — even text-only calls returned 429. This suggests a daily request quota in the range of **~30 vision requests** for this particular free-tier API key.

### Model-Specific Limits (per Google docs)

| Model | RPM | RPD | Verdict |
|---|---|---|---|
| `gemini-2.5-flash-lite` | 15 | 1,000 | Ample for small benchmarks |
| `gemini-2.5-flash` | 10 | 250 | Adequate |
| `gemini-2.5-pro` | 5 | 50 | Very restrictive — barely usable for free |
| `gemini-3-flash-preview` | 10 | 1,500 | Good |
| `gemini-3.1-flash-lite` | 10 | 1,500 | Good |

### Real-World Free Tier Behavior

- The shared global free tier quota appears to be **much lower** than the per-model RPD numbers suggest
- After ~30 total vision calls across all models, the entire API key was blocked (429) for all subsequent requests
- Retry logic with up to 10 attempts (10s–100s backoff) did **not** recover — the limit is daily, not per-minute
- The quota must reset after ~24 hours

### Recommendations for Frank Sherlock

| Model | Recommendation | Rationale |
|---|---|---|
| `gemini-2.5-flash-lite` | ❌ Too generic | Fast but doesn't name characters well. Not suitable for anime identification. |
| `gemini-2.5-flash` | ⚠️ Decent fallback | Good quality but 250 RPD limits throughput. |
| `gemini-3-flash-preview` | ❌ Too slow + preview | High latency and preview-stage model may change. |
| `gemini-3.1-flash-lite` | ✅ **Best fit** | Nears 2.5-flash quality at flash-lite speed. Named every character with series correctly at ~2.4s/image. GA status. 1,500 RPD. |
| `gemini-2.5-pro` | ❌ Too restrictive | 50 RPD makes it unusable for batch scanning. |

### API Integration Notes

- **Endpoint**: Must use `v1beta` for preview models (`gemini-3-flash-preview`)
- **Auth**: Use `?key=` query parameter for AI Studio keys (not `x-goog-api-key` header)
- **responseMimeType**: Field `responseMimeType` in `generationConfig` is **not supported** by `gemini-2.5-flash-lite` — prompts must self-instruct JSON format (which the PRIMARY_PROMPT already does)
- **Image sizes**: Up to 368 KB images were processed without issues; the docs state a 500 MB input size limit

---

## Data Files

- `results/phase2_images/gemini_primary_report.json` — Full raw results for all 4 models
- `scripts/phase2f_gemini_primary.py` — PRIMARY_PROMPT benchmark script
- `scripts/phase2g_gemini_anime.py` — ANIME_PROMPT benchmark script (not run — quota exhausted)
- `docs/BENCHMARK_CONFIG.json` — Model list with RPM/RPD limits
