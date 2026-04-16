//! Central registry of environment variable key names used by naraeclaw-providers.
//!
//! All provider code should reference these constants instead of bare string
//! literals so that key names remain consistent and easy to audit or rename.

// ── Universal overrides ──────────────────────────────────────────────────────

/// Universal API key override: checked before any provider-specific key.
pub const NARAECLAW_API_KEY: &str = "NARAECLAW_API_KEY";
pub const ZEROCLAW_API_KEY: &str = "ZEROCLAW_API_KEY";

/// Universal base URL override for the active provider.
/// Provider-specific `*_BASE_URL` vars take precedence when set.
pub const NARAECLAW_PROVIDER_URL: &str = "NARAECLAW_PROVIDER_URL";
pub const ZEROCLAW_PROVIDER_URL: &str = "ZEROCLAW_PROVIDER_URL";

// ── Anthropic ────────────────────────────────────────────────────────────────

pub const ANTHROPIC_API_KEY: &str = "ANTHROPIC_API_KEY";
pub const ANTHROPIC_OAUTH_TOKEN: &str = "ANTHROPIC_OAUTH_TOKEN";
/// Base URL override for the Anthropic API (default: <https://api.anthropic.com>).
pub const ANTHROPIC_BASE_URL: &str = "ANTHROPIC_BASE_URL";

// ── OpenAI ───────────────────────────────────────────────────────────────────

pub const OPENAI_API_KEY: &str = "OPENAI_API_KEY";
/// Base URL override for the OpenAI API (default: <https://api.openai.com/v1>).
pub const OPENAI_BASE_URL: &str = "OPENAI_BASE_URL";

// ── OpenRouter ───────────────────────────────────────────────────────────────

pub const OPENROUTER_API_KEY: &str = "OPENROUTER_API_KEY";
/// Base URL override for the OpenRouter API (default: <https://openrouter.ai/api/v1>).
pub const OPENROUTER_BASE_URL: &str = "OPENROUTER_BASE_URL";

// ── Ollama ───────────────────────────────────────────────────────────────────

pub const OLLAMA_API_KEY: &str = "OLLAMA_API_KEY";
/// Base URL override for Ollama (default: <http://localhost:11434>).
pub const OLLAMA_BASE_URL: &str = "OLLAMA_BASE_URL";

// ── Gemini ───────────────────────────────────────────────────────────────────

pub const GEMINI_API_KEY: &str = "GEMINI_API_KEY";

// ── Azure OpenAI ─────────────────────────────────────────────────────────────

pub const AZURE_OPENAI_API_KEY: &str = "AZURE_OPENAI_API_KEY";

// ── Groq ─────────────────────────────────────────────────────────────────────

pub const GROQ_API_KEY: &str = "GROQ_API_KEY";

// ── MiniMax ──────────────────────────────────────────────────────────────────

pub const MINIMAX_API_KEY: &str = "MINIMAX_API_KEY";
pub const MINIMAX_OAUTH_TOKEN: &str = "MINIMAX_OAUTH_TOKEN";
pub const MINIMAX_OAUTH_REFRESH_TOKEN: &str = "MINIMAX_OAUTH_REFRESH_TOKEN";
pub const MINIMAX_OAUTH_REGION: &str = "MINIMAX_OAUTH_REGION";
pub const MINIMAX_OAUTH_CLIENT_ID: &str = "MINIMAX_OAUTH_CLIENT_ID";

// ── Qwen ─────────────────────────────────────────────────────────────────────

pub const QWEN_OAUTH_TOKEN: &str = "QWEN_OAUTH_TOKEN";
pub const QWEN_OAUTH_REFRESH_TOKEN: &str = "QWEN_OAUTH_REFRESH_TOKEN";
pub const QWEN_OAUTH_RESOURCE_URL: &str = "QWEN_OAUTH_RESOURCE_URL";
pub const QWEN_OAUTH_CLIENT_ID: &str = "QWEN_OAUTH_CLIENT_ID";
