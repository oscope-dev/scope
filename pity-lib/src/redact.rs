use regex::{Regex};

pub struct Redactor {
    patterns: Vec<Regex>
}

const RANDOM_STRING_REGEX: &str = r#"(?:secret|token|key|password|Secret|SECRET|Token|TOKEN|Key|KEY|Password|PASSWORD)\w*['"]?]?\s*(?:=|:|:=)\s*['"` \t]?([A-Za-z0-9+/_\-.~=]{15,80})(?:['"` \t\n]|$)"#;

impl Redactor {
    pub fn new() -> Self {
            let patterns = vec![
                "(?:r|s)k_live_[0-9a-zA-Z]{24}",            // stripe
                "(?:AC[a-z0-9]{32}|SK[a-z0-9]{32})",        // twilio
                "(?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9_]{36}", // github
                "(?:^|\\W)eyJ[A-Za-z0-9-_=]+\\.[A-Za-z0-9-_=]+\\.?[A-Za-z0-9-_.+/=]*?", // jwt
                "xox(?:a|b|p|o|s|r)-(?:\\d+-)+[a-z0-9]+",   // slack
                "https://hooks\\.slack\\.com/services/T[a-zA-Z0-9_]+/B[a-zA-Z0-9_]+/[a-zA-Z0-9_]+", // slack webhooks
                "//.+/:_authToken=[A-Za-z0-9-_]+",            // legacy npm
                "npm_[A-Za-z0-9]{36}",                        // modern npm tokens
                "AccountKey=[a-zA-Z0-9+/=]{88}",              // azure storage
                "SG\\.[a-zA-Z0-9_-]{22}\\.[a-zA-Z0-9_-]{43}", // sendgrid
                "[0-9a-z]{32}-us[0-9]{1,2}",                  // mailchimp
                r"sq0csp-[0-9A-Za-z\\\-_]{43}",               // square
                "AIzaSy[A-Za-z0-9-_]{33}",                    // gcp api key
                "glpat-[A-Za-z0-9_/-]{20,}",                  // gitlab
                "[A-Za-z]+://[A-Za-z0-9-_.~%]+:([A-Za-z0-9-_.~%]+)@[A-Za-z]+\\.[A-Za-z0-9]+", // URLs with passwords
                // Private keys
                "AGE-SECRET-KEY-[A-Z0-9]{59}", // age secret key
                "-----BEGIN DSA PRIVATE KEY-----(?:$|[^-]{63}[^-]*-----END)",
                "-----BEGIN EC PRIVATE KEY-----(?:$|[^-]{63}[^-]*-----END)",
                "-----BEGIN OPENSSH PRIVATE KEY-----(?:$|[^-]{63}[^-]*-----END)",
                "-----BEGIN PGP PRIVATE KEY BLOCK-----(?:$|[^-]{63}[^-]*-----END)",
                "-----BEGIN PRIVATE KEY-----(?:$|[^-]{63}[^-]*-----END)",
                "-----BEGIN RSA PRIVATE KEY-----(?:$|[^-]{63}[^-]*-----END)",
                "-----BEGIN SSH2 ENCRYPTED PRIVATE KEY-----(?:$|[^-]{63}[^-]*-----END)",
                "PuTTY-User-Key-File-2",
                RANDOM_STRING_REGEX,
            ];

        Self {
            patterns: patterns.iter().map(|pattern| Regex::new(pattern).unwrap()).collect()
        }
    }

    pub fn redact_text(&self, haystack: &str) -> String {
        let mut redacted_string: String = haystack.to_string();
        for re in &self.patterns {
            redacted_string = re.replace_all(&redacted_string, "[REDACTED]").to_string()
        }

        redacted_string.to_string()
    }
}

#[test]
fn test_redactor_creations() {
    Redactor::new();
}

#[test]
fn test_redact_gh_api_key() {
    let redactor = Redactor::new();

    let text = "some really
long string that has a ghp_123456789012345678901234567890123456 fake token";

    assert_eq!("some really\nlong string that has a [REDACTED] fake token", redactor.redact_text(text));
}
