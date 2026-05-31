use std::path::Path;
use std::time::Duration;

use serde_json::Value;

pub const GROQ_DEFAULT_MODEL: &str = "meta-llama/llama-4-maverick-17b-128e-instruct";
pub const GROQ_BASE: &str = "https://api.groq.com/openai/v1";

/// Maximum image payload size Groq accepts (4 MB).
const MAX_IMAGE_BYTES: u64 = 4 * 1024 * 1024;

pub struct GroqResponse {
    pub ok: bool,
    pub raw: String,
}

impl GroqResponse {
    pub fn error(msg: String) -> Self {
        Self {
            ok: false,
            raw: msg,
        }
    }
}

fn mime_from_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("tiff" | "tif") => "image/tiff",
        _ => "image/png",
    }
}

/// Maximum retries on HTTP 429 rate-limit responses.
const MAX_RATE_LIMIT_RETRIES: u32 = 3;

/// Call Groq's OpenAI-compatible chat completions endpoint.
/// Automatically retries on 429 rate-limit responses, sleeping for the
/// `Retry-After` duration between attempts.
pub fn groq_generate(
    api_key: &str,
    model: &str,
    prompt: &str,
    image_path: Option<&Path>,
    num_predict: u32,
    timeout_secs: u64,
    json_mode: bool,
) -> GroqResponse {
    let url = format!("{GROQ_BASE}/chat/completions");

    // Build content once — it's the same across retries
    let mut content: Vec<Value> = Vec::new();

    if let Some(img_path) = image_path {
        let bytes = match std::fs::read(img_path) {
            Ok(b) => b,
            Err(e) => return GroqResponse::error(format!("image_read_error: {e}")),
        };

        if bytes.len() as u64 > MAX_IMAGE_BYTES {
            return GroqResponse::error(format!(
                "image_too_large: {} bytes exceeds 4 MB limit",
                bytes.len()
            ));
        }

        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
        let mime = mime_from_path(img_path);
        content.push(serde_json::json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:{mime};base64,{b64}"),
            }
        }));
    }

    content.push(serde_json::json!({
        "type": "text",
        "text": prompt,
    }));

    let mut payload = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": content,
        }],
        "stream": false,
        "max_completion_tokens": num_predict,
        "temperature": 0.3,
    });

    if json_mode {
        payload["response_format"] = serde_json::json!({"type": "json_object"});
    }

    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_recv_body(Some(Duration::from_secs(timeout_secs)))
        .timeout_send_body(Some(Duration::from_secs(30)))
        .build()
        .into();

    let mut last_error = String::new();

    for attempt in 0..=MAX_RATE_LIMIT_RETRIES {
        let mut resp = match agent
            .post(&url)
            .header("Authorization", &format!("Bearer {api_key}"))
            .header("Content-Type", "application/json")
            .send_json(&payload)
        {
            Ok(r) => r,
            Err(e) => {
                last_error = format!("http_error: {e}");
                // Transport-level error (connection refused, DNS, etc.) — no point retrying
                break;
            }
        };

        let status = resp.status();
        let body = resp.body_mut().read_to_string().unwrap_or_default();

        if status.is_success() {
            // Success path
            let parsed: Value = match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(e) => {
                    return GroqResponse::error(format!("json_parse_error: {e} — body: {body}"));
                }
            };

            let text = parsed
                .get("choices")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|c| c.get("message"))
                .and_then(|m| m.get("content"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            return GroqResponse {
                ok: true,
                raw: text,
            };
        }

        // Error handling
        if status == 429 {
            let retry_after = resp
                .headers()
                .get("Retry-After")
                .and_then(|v| v.to_str().ok().map(|s| s.to_string()))
                .unwrap_or_default();

            let wait_secs: u64 = retry_after.parse().unwrap_or(5);

            log::warn!(
                "Groq rate-limited (attempt {}/{}): retrying after {}s",
                attempt + 1,
                MAX_RATE_LIMIT_RETRIES + 1,
                wait_secs,
            );

            last_error = format!("rate_limited: retry_after={retry_after} body={body}");

            if attempt < MAX_RATE_LIMIT_RETRIES {
                std::thread::sleep(Duration::from_secs(wait_secs));
                continue;
            }
        } else if status == 413 {
            return GroqResponse::error(
                "payload_too_large: image + request exceeds 4 MB limit".to_string(),
            );
        } else {
            let msg = if let Ok(parsed) = serde_json::from_str::<Value>(&body) {
                let error_msg = parsed
                    .get("error")
                    .and_then(|e| e.get("message"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("groq_api_error: {error_msg}")
            } else {
                format!("http_error: {status} — {body}")
            };
            return GroqResponse::error(msg);
        }
    }

    GroqResponse::error(format!("rate_limit_exhausted: {last_error}"))
}

/// Whether the API key appears to be configured (not empty and not a placeholder).
/// Groq keys start with `gsk_` and are at least 10 chars.
pub fn is_api_key_configured(key: &str) -> bool {
    !key.is_empty() && key.len() >= 10
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_key_configured_accepts_valid_key() {
        assert!(is_api_key_configured("gsk_valid_key_here_12345"));
    }

    #[test]
    fn api_key_configured_rejects_empty() {
        assert!(!is_api_key_configured(""));
    }

    #[test]
    fn api_key_configured_rejects_short() {
        assert!(!is_api_key_configured("short"));
    }
}
