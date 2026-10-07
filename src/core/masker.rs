use regex::Regex;

/// Fast, regex-based secret and credential masking filter
#[derive(Clone)]
pub struct SecretMasker {
    patterns: Vec<Regex>,
    custom_secrets: Vec<String>,
}

impl Default for SecretMasker {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretMasker {
    pub fn new() -> Self {
        let patterns = vec![
            // Google OAuth Token (ya29...)
            Regex::new(r"ya29\.[a-zA-Z0-9_\-]+").unwrap(),
            // Anthropic API Key (sk-ant...)
            Regex::new(r"sk-ant-[a-zA-Z0-9_\-]+").unwrap(),
            // OpenAI API Key (sk-...)
            Regex::new(r"sk-[a-zA-Z0-9]{20,}").unwrap(),
            // GitHub Token (ghp_..., gho_...)
            Regex::new(r"gh[pousr]_[a-zA-Z0-9]{20,}").unwrap(),
            // Generic Bearer Token
            Regex::new(r"(?i)bearer\s+([a-zA-Z0-9_\-\.]{20,})").unwrap(),
        ];

        Self {
            patterns,
            custom_secrets: Vec::new(),
        }
    }

    /// Registers a custom secret string to be masked
    pub fn add_custom_secret(&mut self, secret: impl Into<String>) {
        let s = secret.into();
        if s.len() >= 6 && !self.custom_secrets.contains(&s) {
            self.custom_secrets.push(s);
        }
    }

    /// Sanitizes text by replacing sensitive patterns with "***MASKED***"
    pub fn mask(&self, input: &str) -> String {
        let mut result = input.to_string();

        // 1. Mask known patterns
        for pattern in &self.patterns {
            result = pattern.replace_all(&result, "***MASKED***").to_string();
        }

        // 2. Mask custom registered secrets
        for secret in &self.custom_secrets {
            result = result.replace(secret, "***MASKED***");
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_google_token() {
        let masker = SecretMasker::new();
        let log = "Using token ya29.a0AfH6SMAxyz1234567890 for auth";
        assert_eq!(masker.mask(log), "Using token ***MASKED*** for auth");
    }

    #[test]
    fn test_mask_custom_secret() {
        let mut masker = SecretMasker::new();
        masker.add_custom_secret("super_secret_password_123");
        let log = "Connecting with pass: super_secret_password_123 now";
        assert_eq!(masker.mask(log), "Connecting with pass: ***MASKED*** now");
    }
}
