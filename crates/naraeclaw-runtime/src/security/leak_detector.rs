//! Credential leak detection for outbound content.
//!
//! Scans outbound messages for potential credential leaks before they are sent,
//! preventing accidental exfiltration of API keys, tokens, passwords, and other
//! sensitive values.
//!
//! Contributed from RustyClaw (MIT licensed).

use base64::{Engine as _, engine::general_purpose};
use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Minimum token length considered for high-entropy detection.
const ENTROPY_TOKEN_MIN_LEN: usize = 24;

/// Result of leak detection.
#[derive(Debug, Clone)]
pub enum LeakResult {
    /// No leaks detected.
    Clean,
    /// Potential leaks detected with redacted versions.
    Detected {
        /// Descriptions of detected leak patterns.
        patterns: Vec<String>,
        /// Content with sensitive values redacted.
        redacted: String,
    },
}

/// Single credential filtering engine for outbound content.
#[derive(Debug, Clone)]
pub struct CredentialFilter {
    /// Sensitivity threshold (0.0-1.0, higher = more aggressive detection).
    sensitivity: f64,
}

impl Default for CredentialFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialFilter {
    /// Create a new leak detector with default sensitivity.
    pub fn new() -> Self {
        Self { sensitivity: 0.7 }
    }

    /// Create a detector with custom sensitivity.
    pub fn with_sensitivity(sensitivity: f64) -> Self {
        Self {
            sensitivity: sensitivity.clamp(0.0, 1.0),
        }
    }

    /// Scan content for potential credential leaks.
    pub fn scan(&self, content: &str) -> LeakResult {
        let mut patterns = Vec::new();
        let mut redacted = content.to_string();

        self.check_encoded_variants(content, &mut patterns, &mut redacted);
        self.scan_direct(content, &mut patterns, &mut redacted);

        if patterns.is_empty() {
            LeakResult::Clean
        } else {
            LeakResult::Detected { patterns, redacted }
        }
    }

    /// Scrub credentials from text while preserving a small prefix for context.
    pub fn scrub_credentials(&self, input: &str) -> String {
        let scrubbed = sensitive_kv_regex()
            .replace_all(input, |caps: &regex::Captures| {
                let full_match = &caps[0];
                let key = &caps[1];
                let val = caps
                    .get(2)
                    .or(caps.get(3))
                    .or(caps.get(4))
                    .map(|m| m.as_str())
                    .unwrap_or("");
                let prefix = if val.len() > 4 {
                    val.char_indices()
                        .nth(4)
                        .map(|(byte_idx, _)| &val[..byte_idx])
                        .unwrap_or(val)
                } else {
                    ""
                };

                if full_match.contains(':') {
                    if full_match.contains('"') {
                        format!("\"{}\": \"{}*[REDACTED]\"", key, prefix)
                    } else {
                        format!("{}: {}*[REDACTED]", key, prefix)
                    }
                } else if full_match.contains('=') {
                    if full_match.contains('"') {
                        format!("{}=\"{}*[REDACTED]\"", key, prefix)
                    } else {
                        format!("{}={}*[REDACTED]", key, prefix)
                    }
                } else {
                    format!("{}: {}*[REDACTED]", key, prefix)
                }
            })
            .to_string();

        let mut patterns = Vec::new();
        let mut redacted = scrubbed.clone();
        self.scan_post_scrub(&scrubbed, &mut patterns, &mut redacted);
        if patterns.is_empty() {
            scrubbed
        } else {
            redacted
        }
    }

    /// Create an incremental filter for content delivered in chunks.
    pub fn stream(&self) -> CredentialFilterStream {
        CredentialFilterStream::new(self.clone())
    }

    fn scan_direct(&self, content: &str, patterns: &mut Vec<String>, redacted: &mut String) {
        self.check_api_keys(content, patterns, redacted);
        self.check_aws_credentials(content, patterns, redacted);
        self.check_generic_secrets(content, patterns, redacted);
        self.check_private_keys(content, patterns, redacted);
        self.check_jwt_tokens(content, patterns, redacted);
        self.check_database_urls(content, patterns, redacted);
        self.check_high_entropy_tokens(content, patterns, redacted);
    }

    fn check_encoded_variants(
        &self,
        content: &str,
        patterns: &mut Vec<String>,
        redacted: &mut String,
    ) {
        self.check_base64_variants(content, patterns, redacted);
        self.check_hex_variants(content, patterns, redacted);
        self.check_url_encoded_variants(content, patterns, redacted);
    }

    fn scan_post_scrub(&self, content: &str, patterns: &mut Vec<String>, redacted: &mut String) {
        self.check_encoded_variants(content, patterns, redacted);
        self.check_api_keys(content, patterns, redacted);
        self.check_aws_credentials(content, patterns, redacted);
        self.check_private_keys(content, patterns, redacted);
        self.check_jwt_tokens(content, patterns, redacted);
        self.check_database_urls(content, patterns, redacted);
        self.check_high_entropy_tokens(content, patterns, redacted);
    }

    /// Check for common API key patterns.
    fn check_api_keys(&self, content: &str, patterns: &mut Vec<String>, redacted: &mut String) {
        static API_KEY_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
        let regexes = API_KEY_PATTERNS.get_or_init(|| {
            vec![
                // Stripe
                (
                    Regex::new(r"sk_(live|test)_[a-zA-Z0-9]{24,}").unwrap(),
                    "Stripe secret key",
                ),
                (
                    Regex::new(r"pk_(live|test)_[a-zA-Z0-9]{24,}").unwrap(),
                    "Stripe publishable key",
                ),
                // OpenAI
                (
                    Regex::new(r"sk-[a-zA-Z0-9]{20,}T3BlbkFJ[a-zA-Z0-9]{20,}").unwrap(),
                    "OpenAI API key",
                ),
                (
                    Regex::new(r"sk-[a-zA-Z0-9]{48,}").unwrap(),
                    "OpenAI-style API key",
                ),
                // Anthropic
                (
                    Regex::new(r"sk-ant-[a-zA-Z0-9-_]{32,}").unwrap(),
                    "Anthropic API key",
                ),
                // Google
                (
                    Regex::new(r"AIza[a-zA-Z0-9_-]{35}").unwrap(),
                    "Google API key",
                ),
                // GitHub
                (
                    Regex::new(r"gh[pousr]_[a-zA-Z0-9]{36,}").unwrap(),
                    "GitHub token",
                ),
                (
                    Regex::new(r"github_pat_[a-zA-Z0-9_]{22,}").unwrap(),
                    "GitHub PAT",
                ),
                // Generic
                (
                    Regex::new(r#"api[_-]?key[=:]\s*['"]*[a-zA-Z0-9_-]{20,}"#).unwrap(),
                    "Generic API key",
                ),
            ]
        });

        for (regex, name) in regexes {
            if regex.is_match(content) {
                patterns.push(String::from(*name));
                *redacted = regex
                    .replace_all(redacted, "[REDACTED_API_KEY]")
                    .to_string();
            }
        }
    }

    /// Check for AWS credentials.
    fn check_aws_credentials(
        &self,
        content: &str,
        patterns: &mut Vec<String>,
        redacted: &mut String,
    ) {
        static AWS_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
        let regexes = AWS_PATTERNS.get_or_init(|| {
            vec![
                (
                    Regex::new(r"AKIA[A-Z0-9]{16}").unwrap(),
                    "AWS Access Key ID",
                ),
                (
                    Regex::new(
                        r#"aws[_-]?secret[_-]?access[_-]?key[=:]\s*['"]*[a-zA-Z0-9/+=]{40}"#,
                    )
                    .unwrap(),
                    "AWS Secret Access Key",
                ),
            ]
        });

        for (regex, name) in regexes {
            if regex.is_match(content) {
                patterns.push(String::from(*name));
                *redacted = regex
                    .replace_all(redacted, "[REDACTED_AWS_CREDENTIAL]")
                    .to_string();
            }
        }
    }

    /// Check for generic secret patterns.
    fn check_generic_secrets(
        &self,
        content: &str,
        patterns: &mut Vec<String>,
        redacted: &mut String,
    ) {
        static SECRET_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
        let regexes = SECRET_PATTERNS.get_or_init(|| {
            vec![
                (
                    Regex::new(r#"(?i)password[=:]\s*['"]*[^\s'"]{8,}"#).unwrap(),
                    "Password in config",
                ),
                (
                    Regex::new(r#"(?i)secret[=:]\s*['"]*[a-zA-Z0-9_-]{16,}"#).unwrap(),
                    "Secret value",
                ),
                (
                    Regex::new(r#"(?i)token[=:]\s*['"]*[a-zA-Z0-9_.-]{20,}"#).unwrap(),
                    "Token value",
                ),
            ]
        });

        for (regex, name) in regexes {
            if regex.is_match(content) && self.sensitivity > 0.5 {
                patterns.push(String::from(*name));
                *redacted = regex.replace_all(redacted, "[REDACTED_SECRET]").to_string();
            }
        }
    }

    /// Check for private keys.
    fn check_private_keys(&self, content: &str, patterns: &mut Vec<String>, redacted: &mut String) {
        // PEM-encoded private keys
        let key_patterns = [
            (
                "-----BEGIN RSA PRIVATE KEY-----",
                "-----END RSA PRIVATE KEY-----",
                "RSA private key",
            ),
            (
                "-----BEGIN EC PRIVATE KEY-----",
                "-----END EC PRIVATE KEY-----",
                "EC private key",
            ),
            (
                "-----BEGIN PRIVATE KEY-----",
                "-----END PRIVATE KEY-----",
                "Private key",
            ),
            (
                "-----BEGIN OPENSSH PRIVATE KEY-----",
                "-----END OPENSSH PRIVATE KEY-----",
                "OpenSSH private key",
            ),
        ];

        for (begin, end, name) in key_patterns {
            if content.contains(begin) && content.contains(end) {
                patterns.push(name.to_string());
                // Redact the entire key block
                if let Some(start_idx) = content.find(begin)
                    && let Some(end_idx) = content.find(end)
                {
                    let key_block = &content[start_idx..end_idx + end.len()];
                    *redacted = redacted.replace(key_block, "[REDACTED_PRIVATE_KEY]");
                }
            }
        }
    }

    /// Check for JWT tokens.
    fn check_jwt_tokens(&self, content: &str, patterns: &mut Vec<String>, redacted: &mut String) {
        static JWT_PATTERN: OnceLock<Regex> = OnceLock::new();
        let regex = JWT_PATTERN.get_or_init(|| {
            // JWT: three base64url-encoded parts separated by dots
            Regex::new(r"eyJ[a-zA-Z0-9_-]*\.eyJ[a-zA-Z0-9_-]*\.[a-zA-Z0-9_-]*").unwrap()
        });

        if regex.is_match(content) {
            patterns.push("JWT token".to_string());
            *redacted = regex.replace_all(redacted, "[REDACTED_JWT]").to_string();
        }
    }

    /// Check for database connection URLs.
    fn check_database_urls(
        &self,
        content: &str,
        patterns: &mut Vec<String>,
        redacted: &mut String,
    ) {
        static DB_PATTERNS: OnceLock<Vec<(Regex, &'static str)>> = OnceLock::new();
        let regexes = DB_PATTERNS.get_or_init(|| {
            vec![
                (
                    Regex::new(r"postgres(ql)?://[^:]+:[^@]+@[^\s]+").unwrap(),
                    "PostgreSQL connection URL",
                ),
                (
                    Regex::new(r"mysql://[^:]+:[^@]+@[^\s]+").unwrap(),
                    "MySQL connection URL",
                ),
                (
                    Regex::new(r"mongodb(\+srv)?://[^:]+:[^@]+@[^\s]+").unwrap(),
                    "MongoDB connection URL",
                ),
                (
                    Regex::new(r"redis://[^:]+:[^@]+@[^\s]+").unwrap(),
                    "Redis connection URL",
                ),
            ]
        });

        for (regex, name) in regexes {
            if regex.is_match(content) {
                patterns.push(String::from(*name));
                *redacted = regex
                    .replace_all(redacted, "[REDACTED_DATABASE_URL]")
                    .to_string();
            }
        }
    }

    /// Check for high-entropy tokens that may be leaked credentials.
    ///
    /// Extracts candidate tokens from content (after stripping URLs to avoid
    /// false-positives on path segments) and flags any that exceed the Shannon
    /// entropy threshold derived from the detector's sensitivity.
    fn check_high_entropy_tokens(
        &self,
        content: &str,
        patterns: &mut Vec<String>,
        redacted: &mut String,
    ) {
        // Entropy threshold scales with sensitivity: at 0.7 this is ~4.37.
        let entropy_threshold = 3.5 + self.sensitivity * 1.25;

        // Strip URLs and media markers before extracting tokens so that path
        // segments are not mistaken for high-entropy credentials.
        // Media markers like [IMAGE:/path/to/file.png] contain filesystem paths
        // that look like high-entropy tokens when `/` is included in the token
        // character set (#4604).
        static URL_PATTERN: OnceLock<Regex> = OnceLock::new();
        let url_re = URL_PATTERN.get_or_init(|| Regex::new(r"https?://\S+").unwrap());
        static MEDIA_MARKER_PATTERN: OnceLock<Regex> = OnceLock::new();
        let media_re = MEDIA_MARKER_PATTERN.get_or_init(|| {
            Regex::new(r"\[(IMAGE|VIDEO|VOICE|AUDIO|DOCUMENT|FILE):[^\]]*\]").unwrap()
        });
        let content_stripped = url_re.replace_all(content, "");
        let content_without_urls = media_re.replace_all(&content_stripped, "");

        let tokens = extract_candidate_tokens(&content_without_urls);

        for token in tokens {
            if token.len() >= ENTROPY_TOKEN_MIN_LEN {
                let entropy = shannon_entropy(token);
                if entropy >= entropy_threshold && has_mixed_alpha_digit(token) {
                    patterns.push("High-entropy token".to_string());
                    *redacted = redacted.replace(token, "[REDACTED_HIGH_ENTROPY_TOKEN]");
                }
            }
        }
    }

    fn check_base64_variants(
        &self,
        content: &str,
        patterns: &mut Vec<String>,
        redacted: &mut String,
    ) {
        static BASE64_PATTERN: OnceLock<Regex> = OnceLock::new();
        let regex =
            BASE64_PATTERN.get_or_init(|| Regex::new(r"\b[A-Za-z0-9+/_-]{24,}={0,2}\b").unwrap());

        for mat in regex.find_iter(content) {
            let candidate = mat.as_str();
            if !candidate.ends_with('=') && !has_encoding_context(content, mat.start(), "base64") {
                continue;
            }
            if let Some(decoded) = decode_base64_candidate(candidate)
                && let Some(mut decoded_patterns) = self.detect_direct_patterns(&decoded)
            {
                patterns.push("Base64-encoded credential".to_string());
                patterns.append(&mut decoded_patterns);
                *redacted = redacted.replace(candidate, "[REDACTED_ENCODED_CREDENTIAL]");
            }
        }
    }

    fn check_hex_variants(&self, content: &str, patterns: &mut Vec<String>, redacted: &mut String) {
        static HEX_PATTERN: OnceLock<Regex> = OnceLock::new();
        let regex = HEX_PATTERN.get_or_init(|| Regex::new(r"(?i)\b[0-9a-f]{32,}\b").unwrap());

        for mat in regex.find_iter(content) {
            let candidate = mat.as_str();
            if !has_encoding_context(content, mat.start(), "hex") {
                continue;
            }
            if candidate.len() % 2 != 0 {
                continue;
            }
            if let Ok(bytes) = hex::decode(candidate)
                && let Ok(decoded) = String::from_utf8(bytes)
                && is_mostly_printable(&decoded)
                && let Some(mut decoded_patterns) = self.detect_direct_patterns(&decoded)
            {
                patterns.push("Hex-encoded credential".to_string());
                patterns.append(&mut decoded_patterns);
                *redacted = redacted.replace(candidate, "[REDACTED_ENCODED_CREDENTIAL]");
            }
        }
    }

    fn check_url_encoded_variants(
        &self,
        content: &str,
        patterns: &mut Vec<String>,
        redacted: &mut String,
    ) {
        static URL_ENCODED_PATTERN: OnceLock<Regex> = OnceLock::new();
        let regex = URL_ENCODED_PATTERN
            .get_or_init(|| Regex::new(r"(?:%[0-9A-Fa-f]{2}|[A-Za-z0-9._~+\-]){12,}").unwrap());

        for candidate in regex.find_iter(content).map(|m| m.as_str()) {
            if !candidate.contains('%') {
                continue;
            }
            if let Ok(decoded) = urlencoding::decode(candidate)
                && let Some(mut decoded_patterns) = self.detect_direct_patterns(&decoded)
            {
                patterns.push("URL-encoded credential".to_string());
                patterns.append(&mut decoded_patterns);
                *redacted = redacted.replace(candidate, "[REDACTED_ENCODED_CREDENTIAL]");
            }
        }
    }

    fn detect_direct_patterns(&self, content: &str) -> Option<Vec<String>> {
        let mut patterns = Vec::new();
        let mut redacted = content.to_string();
        self.scan_direct(content, &mut patterns, &mut redacted);
        (!patterns.is_empty()).then_some(patterns)
    }
}

/// Backward-compatible leak detector facade backed by [`CredentialFilter`].
#[derive(Debug, Clone)]
pub struct LeakDetector {
    filter: CredentialFilter,
}

impl Default for LeakDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl LeakDetector {
    /// Create a new leak detector with default sensitivity.
    pub fn new() -> Self {
        Self {
            filter: CredentialFilter::new(),
        }
    }

    /// Create a detector with custom sensitivity.
    pub fn with_sensitivity(sensitivity: f64) -> Self {
        Self {
            filter: CredentialFilter::with_sensitivity(sensitivity),
        }
    }

    /// Scan content for potential credential leaks.
    pub fn scan(&self, content: &str) -> LeakResult {
        self.filter.scan(content)
    }
}

/// Incremental credential filter for content assembled from streamed chunks.
#[derive(Debug, Clone)]
pub struct CredentialFilterStream {
    filter: CredentialFilter,
    buffer: String,
}

impl CredentialFilterStream {
    fn new(filter: CredentialFilter) -> Self {
        Self {
            filter,
            buffer: String::new(),
        }
    }

    /// Add the next chunk to the stream buffer.
    pub fn push_chunk(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
    }

    /// Scan all chunks seen so far and return the filtered content.
    pub fn finish(self) -> String {
        match self.filter.scan(&self.buffer) {
            LeakResult::Clean => self.buffer,
            LeakResult::Detected { redacted, .. } => redacted,
        }
    }
}

fn has_encoding_context(content: &str, candidate_start: usize, label: &str) -> bool {
    let start = content[..candidate_start]
        .char_indices()
        .rev()
        .nth(32)
        .map(|(idx, _)| idx)
        .unwrap_or(0);
    content[start..candidate_start]
        .to_ascii_lowercase()
        .contains(label)
}

fn sensitive_kv_regex() -> &'static Regex {
    static SENSITIVE_KV_REGEX: OnceLock<Regex> = OnceLock::new();
    SENSITIVE_KV_REGEX.get_or_init(|| {
        Regex::new(r#"(?i)(token|api[_-]?key|password|secret|user[_-]?key|bearer|credential)["']?\s*[:=]\s*(?:"([^"]{8,})"|'([^']{8,})'|([a-zA-Z0-9_\-\.]{8,}))"#).unwrap()
    })
}

fn decode_base64_candidate(candidate: &str) -> Option<String> {
    let engines = [
        &general_purpose::STANDARD,
        &general_purpose::STANDARD_NO_PAD,
        &general_purpose::URL_SAFE,
        &general_purpose::URL_SAFE_NO_PAD,
    ];

    for engine in engines {
        if let Ok(bytes) = engine.decode(candidate)
            && let Ok(decoded) = String::from_utf8(bytes)
            && is_mostly_printable(&decoded)
        {
            return Some(decoded);
        }
    }
    None
}

fn is_mostly_printable(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| !c.is_control() || c == '\n' || c == '\r' || c == '\t')
}

/// Extract candidate tokens by splitting on characters outside the
/// alphanumeric + common credential character set.
fn extract_candidate_tokens(content: &str) -> Vec<&str> {
    content
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-' && c != '+' && c != '/')
        .filter(|s| !s.is_empty())
        .collect()
}

/// Compute Shannon entropy (bits per character) for the given string.
fn shannon_entropy(s: &str) -> f64 {
    let len = s.len() as f64;
    if len == 0.0 {
        return 0.0;
    }
    let mut freq: HashMap<u8, usize> = HashMap::new();
    for &b in s.as_bytes() {
        *freq.entry(b).or_insert(0) += 1;
    }
    freq.values().fold(0.0, |acc, &count| {
        let p = count as f64 / len;
        acc - p * p.log2()
    })
}

/// Check whether a token contains both alphabetic and digit characters.
fn has_mixed_alpha_digit(s: &str) -> bool {
    let has_alpha = s.bytes().any(|b| b.is_ascii_alphabetic());
    let has_digit = s.bytes().any(|b| b.is_ascii_digit());
    has_alpha && has_digit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_content_passes() {
        let detector = LeakDetector::new();
        let result = detector.scan("This is just some normal text");
        assert!(matches!(result, LeakResult::Clean));
    }

    #[test]
    fn detects_stripe_keys() {
        let detector = LeakDetector::new();
        let content = "My Stripe key is sk_test_1234567890abcdefghijklmnop";
        let result = detector.scan(content);
        match result {
            LeakResult::Detected { patterns, redacted } => {
                assert!(patterns.iter().any(|p| p.contains("Stripe")));
                assert!(redacted.contains("[REDACTED"));
            }
            LeakResult::Clean => panic!("Should detect Stripe key"),
        }
    }

    #[test]
    fn detects_aws_credentials() {
        let detector = LeakDetector::new();
        let content = "AWS key: AKIAIOSFODNN7EXAMPLE";
        let result = detector.scan(content);
        match result {
            LeakResult::Detected { patterns, .. } => {
                assert!(patterns.iter().any(|p| p.contains("AWS")));
            }
            LeakResult::Clean => panic!("Should detect AWS key"),
        }
    }

    #[test]
    fn detects_private_keys() {
        let detector = LeakDetector::new();
        let content = r#"
-----BEGIN RSA PRIVATE KEY-----
MIIEowIBAAKCAQEA0ZPr5JeyVDonXsKhfq...
-----END RSA PRIVATE KEY-----
"#;
        let result = detector.scan(content);
        match result {
            LeakResult::Detected { patterns, redacted } => {
                assert!(patterns.iter().any(|p| p.contains("private key")));
                assert!(redacted.contains("[REDACTED_PRIVATE_KEY]"));
            }
            LeakResult::Clean => panic!("Should detect private key"),
        }
    }

    #[test]
    fn detects_jwt_tokens() {
        let detector = LeakDetector::new();
        let content = "Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.dozjgNryP4J3jVmNHl0w5N_XgL0n3I9PlFUP0THsR8U";
        let result = detector.scan(content);
        match result {
            LeakResult::Detected { patterns, redacted } => {
                assert!(patterns.iter().any(|p| p.contains("JWT")));
                assert!(redacted.contains("[REDACTED_JWT]"));
            }
            LeakResult::Clean => panic!("Should detect JWT"),
        }
    }

    #[test]
    fn detects_database_urls() {
        let detector = LeakDetector::new();
        let content = "DATABASE_URL=postgres://user:secretpassword@localhost:5432/mydb";
        let result = detector.scan(content);
        match result {
            LeakResult::Detected { patterns, .. } => {
                assert!(patterns.iter().any(|p| p.contains("PostgreSQL")));
            }
            LeakResult::Clean => panic!("Should detect database URL"),
        }
    }

    #[test]
    fn detects_base64_encoded_credentials() {
        let filter = CredentialFilter::new();
        let secret = "api_key=sk-ant-abcdefghijklmnopqrstuvwxyz123456";
        let encoded = general_purpose::STANDARD.encode(secret);
        let content = format!("base64 secret: {encoded}");
        let result = filter.scan(&content);
        match result {
            LeakResult::Detected { patterns, redacted } => {
                assert!(patterns.iter().any(|p| p.contains("Base64")));
                assert!(!redacted.contains(&encoded));
                assert!(redacted.contains("[REDACTED_ENCODED_CREDENTIAL]"));
            }
            LeakResult::Clean => panic!("Should detect base64-encoded credential"),
        }
    }

    #[test]
    fn detects_hex_encoded_credentials() {
        let filter = CredentialFilter::new();
        let secret = "token=sk-ant-abcdefghijklmnopqrstuvwxyz123456";
        let encoded = hex::encode(secret);
        let content = format!("hex secret: {encoded}");
        let result = filter.scan(&content);
        match result {
            LeakResult::Detected { patterns, redacted } => {
                assert!(patterns.iter().any(|p| p.contains("Hex")));
                assert!(!redacted.contains(&encoded));
                assert!(redacted.contains("[REDACTED_ENCODED_CREDENTIAL]"));
            }
            LeakResult::Clean => panic!("Should detect hex-encoded credential"),
        }
    }

    #[test]
    fn detects_url_encoded_credentials() {
        let filter = CredentialFilter::new();
        let secret = "password=supersecret123456";
        let encoded = urlencoding::encode(secret).into_owned();
        let content = format!("url secret: {encoded}");
        let result = filter.scan(&content);
        match result {
            LeakResult::Detected { patterns, redacted } => {
                assert!(patterns.iter().any(|p| p.contains("URL")));
                assert!(!redacted.contains(&encoded));
                assert!(redacted.contains("[REDACTED_ENCODED_CREDENTIAL]"));
            }
            LeakResult::Clean => panic!("Should detect URL-encoded credential"),
        }
    }

    #[test]
    fn stream_filter_detects_credentials_split_across_chunks() {
        let mut stream = CredentialFilter::new().stream();
        stream.push_chunk("api_key=sk-ant-abcdefghijkl");
        stream.push_chunk("mnopqrstuvwxyz123456");

        let filtered = stream.finish();

        assert!(!filtered.contains("sk-ant-abcdefghijklmnopqrstuvwxyz123456"));
        assert!(filtered.contains("[REDACTED"));
    }

    #[test]
    fn scrub_credentials_uses_credential_filter_engine() {
        let filter = CredentialFilter::new();
        let input = "API_KEY=sk-1234567890abcdef and AKIAIOSFODNN7EXAMPLE";
        let scrubbed = filter.scrub_credentials(input);

        assert!(scrubbed.contains("API_KEY=sk-1*[REDACTED]"));
        assert!(!scrubbed.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn low_sensitivity_skips_generic() {
        let detector = LeakDetector::with_sensitivity(0.3);
        let content = "secret=mygenericvalue123456";
        let result = detector.scan(content);
        // Low sensitivity should not flag generic secrets
        assert!(matches!(result, LeakResult::Clean));
    }

    #[test]
    fn url_path_segments_not_flagged() {
        let detector = LeakDetector::new();
        // URL with a long mixed-alphanumeric path segment that would previously
        // false-positive as a high-entropy token.
        let content =
            "See https://example.org/documents/2024-report-a1b2c3d4e5f6g7h8i9j0.pdf for details";
        let result = detector.scan(content);
        assert!(
            matches!(result, LeakResult::Clean),
            "URL path segments should not trigger high-entropy detection"
        );
    }

    #[test]
    fn url_with_long_path_not_redacted() {
        let detector = LeakDetector::new();
        let content = "Reference: https://gov.example.com/publications/research/2024-annual-fiscal-policy-review-9a8b7c6d5e4f3g2h1i0j.html";
        let result = detector.scan(content);
        assert!(
            matches!(result, LeakResult::Clean),
            "Long URL paths should not be redacted"
        );
    }

    #[test]
    fn media_markers_not_redacted_as_high_entropy() {
        let detector = LeakDetector::new();
        let content = "Here is the image: [IMAGE:/Users/matt/.zeroclaw/workspace/skills/image-gen/images/20260324_135911.png]";
        let result = detector.scan(content);
        assert!(
            matches!(result, LeakResult::Clean),
            "Local media markers should not be redacted"
        );
    }

    #[test]
    fn detects_high_entropy_token_outside_url() {
        let detector = LeakDetector::new();
        // A standalone high-entropy token (not in a URL) should still be detected.
        let content = "Found credential: aB3xK9mW2pQ7vL4nR8sT1yU6hD0jF5cG";
        let result = detector.scan(content);
        match result {
            LeakResult::Detected { patterns, redacted } => {
                assert!(patterns.iter().any(|p| p.contains("High-entropy")));
                assert!(redacted.contains("[REDACTED_HIGH_ENTROPY_TOKEN]"));
            }
            LeakResult::Clean => panic!("Should detect high-entropy token"),
        }
    }

    #[test]
    fn low_sensitivity_raises_entropy_threshold() {
        let detector = LeakDetector::with_sensitivity(0.3);
        // At low sensitivity the entropy threshold is higher (3.5 + 0.3*1.25 = 3.875).
        // A repetitive mixed token has low entropy and should not be flagged.
        let content = "token found: ab12ab12ab12ab12ab12ab12ab12ab12";
        let result = detector.scan(content);
        assert!(
            matches!(result, LeakResult::Clean),
            "Low-entropy repetitive tokens should not be flagged"
        );
    }

    #[test]
    fn extract_candidate_tokens_splits_correctly() {
        let tokens = extract_candidate_tokens("foo.bar:baz qux-quux key=val");
        assert!(tokens.contains(&"foo"));
        assert!(tokens.contains(&"bar"));
        assert!(tokens.contains(&"baz"));
        assert!(tokens.contains(&"qux-quux"));
        // '=' is a delimiter, not part of tokens
        assert!(tokens.contains(&"key"));
        assert!(tokens.contains(&"val"));
    }

    #[test]
    fn media_marker_image_path_not_redacted() {
        let detector = LeakDetector::new();
        let content = "Here is your image: [IMAGE:/Users/matt/.zeroclaw/workspace/skills/image-gen/images/20260324_135911.png]";
        let result = detector.scan(content);
        assert!(
            matches!(result, LeakResult::Clean),
            "Media marker image paths should not trigger high-entropy detection"
        );
    }

    #[test]
    fn media_marker_video_not_redacted() {
        let detector = LeakDetector::new();
        let content = "Attached: [VIDEO:/path/to/long/video/file/name123456.mp4]";
        let result = detector.scan(content);
        assert!(
            matches!(result, LeakResult::Clean),
            "Media marker video paths should not trigger high-entropy detection"
        );
    }

    #[test]
    fn actual_high_entropy_still_detected() {
        let detector = LeakDetector::new();
        let content = "Leaked credential: aB3xK9mW2pQ7vL4nR8sT1yU6hD0jF5cG";
        let result = detector.scan(content);
        match result {
            LeakResult::Detected { patterns, redacted } => {
                assert!(patterns.iter().any(|p| p.contains("High-entropy")));
                assert!(redacted.contains("[REDACTED_HIGH_ENTROPY_TOKEN]"));
            }
            LeakResult::Clean => {
                panic!("Should still detect high-entropy tokens outside media markers")
            }
        }
    }

    #[test]
    fn shannon_entropy_empty_string() {
        assert_eq!(shannon_entropy(""), 0.0);
    }

    #[test]
    fn shannon_entropy_single_char() {
        // All same characters: entropy = 0
        assert_eq!(shannon_entropy("aaaa"), 0.0);
    }

    #[test]
    fn shannon_entropy_two_equal_chars() {
        // "ab" repeated: entropy = 1.0 bit
        let e = shannon_entropy("abab");
        assert!((e - 1.0).abs() < 0.001);
    }
}
