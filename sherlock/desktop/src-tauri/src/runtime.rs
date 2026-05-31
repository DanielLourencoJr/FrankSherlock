use crate::models::RuntimeStatus;
use crate::platform::gpu::GpuInfo;

pub fn gather_runtime_status(gpu: &GpuInfo) -> RuntimeStatus {
    let settings = crate::config::load_runtime_settings().ok();
    let provider_name = settings
        .as_ref()
        .and_then(|s| s.provider.as_deref())
        .unwrap_or("ollama");

    let (ollama_available, loaded_models, current_model) = match provider_name {
        "groq" => {
            let groq_api_key = settings
                .as_ref()
                .and_then(crate::config::resolve_groq_api_key);
            let model = settings
                .as_ref()
                .and_then(|s| s.groq_model.as_deref())
                .unwrap_or(crate::llm::GROQ_DEFAULT_MODEL);
            let configured = groq_api_key
                .as_ref()
                .map(|k| crate::llm::is_groq_key_configured(k))
                .unwrap_or(false);
            (
                false,
                Vec::new(),
                if configured {
                    Some(model.to_string())
                } else {
                    None
                },
            )
        }
        "openrouter" => {
            let api_key = settings
                .as_ref()
                .and_then(crate::config::resolve_openrouter_api_key);
            let model = settings
                .as_ref()
                .and_then(|s| s.openrouter_model.as_deref())
                .unwrap_or(crate::llm::OPENROUTER_DEFAULT_MODEL);
            let configured = api_key
                .as_ref()
                .map(|k| crate::llm::is_openrouter_key_configured(k))
                .unwrap_or(false);
            (
                false,
                Vec::new(),
                if configured {
                    Some(model.to_string())
                } else {
                    None
                },
            )
        }
        _ => {
            let (available, loaded) = crate::llm::list_loaded_models();
            (available, loaded.clone(), loaded.first().cloned())
        }
    };

    RuntimeStatus {
        os: crate::platform::current_os(),
        current_model,
        loaded_models,
        vram_used_mib: gpu.vram_used_mib,
        vram_total_mib: gpu.vram_total_mib,
        gpu_vendor: gpu.vendor,
        unified_memory: gpu.unified_memory,
        system_ram_mib: gpu.system_ram_mib,
        ollama_available,
        provider: provider_name.to_string(),
    }
}
