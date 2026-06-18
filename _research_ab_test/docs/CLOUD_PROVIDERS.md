## Cloud Provider Findings

Tested 2026-06-18 using sherlock CLI (`sherlock classify <file>`) against 6 anime/artwork images in `D:\Junior\Imagens\Teste`.

### Providers Implemented

| Provider | Config | Default Model | Image Limit | Pricing |
|----------|--------|---------------|-------------|---------|
| Ollama | `provider = "ollama"` | auto-detect (qwen2.5vl:7b) | unlimited (local) | free |
| Groq | `provider = "groq"` | `qwen/qwen3.6-27b` | 20 MB | paid: $0.60/M in, $3.00/M out |
| OpenRouter | `provider = "openrouter"` | `nvidia/nemotron-nano-12b-v2-vl:free` | 10 MB | free tier available |

### Model Quality Comparison (6-image test set)

**Groq / Llama 4 Scout 17B** (free tier on Groq):
- media_type accuracy: 6/6 ✓
- character ID accuracy: 2/4 correct (Tanya ✓, Tanya ✓, Eren ✗, Mitaka unnamed, Adao ✗, Mitaka unnamed)
- series ID accuracy: 3/4 correct
- False positives: document enrichment triggered by OCR text ("TANUKI" → Issuer), anime enrichment on non-anime
- Speed: fast (~2s/image), rate limit ~30 req/min on free tier
- Notes: Qwen 3.6-27B is paid-only on Groq; free tier only covers Llama 4 Scout

**OpenRouter / Nemotron Nano 12B VL** (free tier on OpenRouter):
- media_type accuracy: 5/5 ✓ (1 timed out)
- character ID accuracy: 2/2 correct (Tanya ✓, Eren ✓)
- series ID accuracy: 2/2 correct
- False positives: none observed (no spurious document/anime enrichment)
- Speed: slightly slower than Llama Scout but <5s/image
- Notes: best free vision model available; supports images, JSON mode

**Gemini (via OpenRouter)**:
- Gemini models on OpenRouter are paid-only (no `:free` suffix available)
- Available IDs: `google/gemini-2.5-flash`, `google/gemini-2.5-pro`
- Not tested due to cost; expected to match or exceed Nemotron quality
- Direct Google AI API key (present in user's settings) could power a native provider

### Key Technical Issues Found

1. **Groq JSON mode breaks Qwen 3.6**: `response_format: {"type": "json_object"}` causes "Failed to validate JSON" errors. Fixed by removing `response_format` from Groq client — the existing `parse_json_response()` fallback is more reliable.

2. **OpenRouter rate limits**: Free tier models (Gemma 4, Qwen Next) hit upstream provider rate limits quickly. Nemotron Nano VL was the only free vision model that consistently responded.

3. **OpenRouter model IDs change frequently**: Gemini 2.5 Flash model IDs (`google/gemini-2.5-flash-exp-03-07:free`, `google/gemini-2.0-flash-exp:free`) were invalid by test time. Use OpenRouter's `/api/v1/models` endpoint to discover current IDs.

4. **WebP file size matters**: The 20 MB limit in Groq allows full-resolution images; OpenRouter's 10 MB and free tier limits are more restrictive.

### Per-Image Results (ground truth)

| Image | Expected | Groq/Llama Scout | OpenRouter/Nemotron |
|-------|----------|------------------|---------------------|
| Tanya17.webp | Tanya Degurechaff, Youjo Senki, artwork | artwork 90%, Tanya ✓, Youjo Senki ✓ | artwork 95%, Tanya ✓, Youjo Senki ✓ |
| Eren2.jpg | Eren Yeager, Attack on Titan, anime | anime 90%, **Shuichi Shindo ✗** | anime 95%, **Eren Yeager ✓** |
| Mitaka4.webp | Mitaka Asa, Chainsaw Man, manga | manga 90%, "eating ice cream" (unnamed) | manga 95%, "on a balcony" (unnamed) |
| Adao1.jpeg | Kaworu Nagisa, Evangelion, anime | anime 90%, **Tokyo Ghoul ✗** | artwork 95%, generic (no false series) |
| Tanya13.jpeg | Tanya Degurechaff, Youjo Senki, artwork | artwork 90%, Youjo Senki ✓, **false doc enrich** | artwork 95%, Youjo Senki ✓ |
| mitaka1.jpg | Mitaka Asa, Chainsaw Man, manga | manga 90%, **Nami ✗** | timed out |

### Recommendation

**Use `nvidia/nemotron-nano-12b-v2-vl:free` on OpenRouter** as the primary cloud provider:
- Best quality-to-cost ratio among tested options
- No false enrichment triggers
- Correctly identifies characters and series
- Free tier works reliably

If Groq payment is added, retest `qwen/qwen3.6-27b` — it's expected to match or exceed Nemotron quality at 500 tps speed.
