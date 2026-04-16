//! Provider alias functions used by config validation.
//!
//! These are extracted from the providers module to break the circular
//! dependency between config and providers.

pub fn is_glm_global_alias(name: &str) -> bool {
    matches!(name, "glm" | "zhipu" | "glm-global" | "zhipu-global")
}

pub fn is_glm_alias(name: &str) -> bool {
    is_glm_global_alias(name)
}

pub fn is_zai_global_alias(name: &str) -> bool {
    matches!(name, "zai" | "z.ai" | "zai-global" | "z.ai-global")
}

pub fn is_zai_alias(name: &str) -> bool {
    is_zai_global_alias(name)
}

pub fn is_minimax_intl_alias(name: &str) -> bool {
    matches!(
        name,
        "minimax"
            | "minimax-intl"
            | "minimax-io"
            | "minimax-global"
            | "minimax-oauth"
            | "minimax-portal"
            | "minimax-oauth-global"
            | "minimax-portal-global"
    )
}

pub fn is_minimax_alias(name: &str) -> bool {
    is_minimax_intl_alias(name)
}

pub fn is_moonshot_intl_alias(name: &str) -> bool {
    matches!(
        name,
        "moonshot" | "kimi" | "moonshot-intl" | "moonshot-global" | "kimi-intl" | "kimi-global"
    )
}

pub fn is_moonshot_alias(name: &str) -> bool {
    is_moonshot_intl_alias(name)
}

pub fn is_qwen_intl_alias(name: &str) -> bool {
    matches!(
        name,
        "qwen"
            | "dashscope"
            | "qwen-intl"
            | "dashscope-intl"
            | "qwen-international"
            | "dashscope-international"
    )
}

pub fn is_qwen_us_alias(name: &str) -> bool {
    matches!(name, "qwen-us" | "dashscope-us")
}

pub fn is_qwen_oauth_alias(name: &str) -> bool {
    matches!(name, "qwen-code" | "qwen-oauth" | "qwen_oauth")
}

pub fn is_qwen_alias(name: &str) -> bool {
    is_qwen_intl_alias(name) || is_qwen_us_alias(name) || is_qwen_oauth_alias(name)
}

pub fn canonical_provider_family(name: &str) -> Option<&'static str> {
    if is_qwen_alias(name) {
        Some("qwen")
    } else if is_glm_alias(name) {
        Some("glm")
    } else if is_moonshot_alias(name) {
        Some("moonshot")
    } else if is_minimax_alias(name) {
        Some("minimax")
    } else if is_zai_alias(name) {
        Some("zai")
    } else {
        None
    }
}
