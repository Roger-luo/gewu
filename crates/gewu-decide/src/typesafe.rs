//! TypeSafe System-One: `POST <url>` with
//! `{model, state, questions: {<name>: {type: "noul", instructions, criteria}}}`,
//! answered by `answers.<name>.noul`, a probability. Every criterion goes in
//! one request, so a chunk costs one call.
//!
//! 429, 5xx, and network errors are retried up to three times with
//! exponential backoff. The key is read when a call is made, sent only as the
//! bearer token, and removed from every error message.

use std::{collections::BTreeMap, time::Duration};

use gewu_review::load::Criterion;
use serde_json::{Value, json};
use ureq::{Agent, http::Request};

use crate::{DecideError, Decider};

pub const DEFAULT_URL: &str = "https://api.typesafe.ai/v1/systemone";
pub const DEFAULT_MODEL: &str = "jev-latest";
/// Holds the API key; only its name is ever printed.
pub const KEY_VARIABLE: &str = "TYPESAFE_API_KEY";

const RETRIES: u32 = 3;
const TIMEOUT: Duration = Duration::from_secs(30);
/// How much of an error body a message keeps.
const BODY_BYTES: usize = 300;

pub struct TypesafeDecider {
    agent: Agent,
    url: String,
    model: String,
    /// Set by tests; otherwise [`KEY_VARIABLE`] is read at each call.
    key: Option<String>,
    backoff: Duration,
}

impl Default for TypesafeDecider {
    fn default() -> Self {
        Self {
            agent: Agent::config_builder()
                .http_status_as_error(false)
                .timeout_global(Some(TIMEOUT))
                .build()
                .new_agent(),
            url: DEFAULT_URL.into(),
            model: DEFAULT_MODEL.into(),
            key: None,
            backoff: Duration::from_millis(500),
        }
    }
}

impl TypesafeDecider {
    /// Talks to `url` with `key`, without waiting between retries; tests use
    /// a loopback server.
    #[must_use]
    pub fn at(url: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            key: Some(key.into()),
            backoff: Duration::ZERO,
            ..Self::default()
        }
    }

    fn key(&self) -> Result<String, DecideError> {
        self.key
            .clone()
            .or_else(|| std::env::var(KEY_VARIABLE).ok())
            .filter(|key| !key.is_empty())
            .ok_or_else(|| DecideError::MissingKey(KEY_VARIABLE.into()))
    }

    fn body(&self, state: &str, criteria: &[Criterion]) -> Value {
        let questions: serde_json::Map<String, Value> = criteria
            .iter()
            .map(|c| {
                let question = json!({
                    "type": "noul",
                    "instructions": c.instructions,
                    "criteria": {"true": c.when_true, "false": c.when_false},
                });
                (c.name.clone(), question)
            })
            .collect();
        json!({"model": self.model, "state": state, "questions": questions})
    }

    /// One exchange: the status and body, or a network error.
    fn post(&self, key: &str, body: &str) -> Result<(u16, String), String> {
        let request = Request::builder()
            .method("POST")
            .uri(&self.url)
            .header("Authorization", format!("Bearer {key}"))
            .header("Content-Type", "application/json")
            .header("User-Agent", "gewu")
            .body(body.to_owned())
            .map_err(|error| error.to_string())?;
        let mut response = self.agent.run(request).map_err(|e| e.to_string())?;
        let status = response.status().as_u16();
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| e.to_string())?;
        Ok((status, text))
    }
}

impl Decider for TypesafeDecider {
    fn answer(
        &self,
        state: &str,
        criteria: &[Criterion],
    ) -> Result<BTreeMap<String, f64>, DecideError> {
        let key = self.key()?;
        let body = self.body(state, criteria).to_string();
        let mut attempt = 0;
        let text = loop {
            let retry = match self.post(&key, &body) {
                Ok((status, text)) if (200..300).contains(&status) => break text,
                Ok((status, text)) if matches!(status, 429 | 500 | 502 | 503 | 504) => {
                    DecideError::Http {
                        status,
                        body: cut(&redact(&text, &key)),
                    }
                }
                Ok((status, text)) => {
                    return Err(DecideError::Http {
                        status,
                        body: cut(&redact(&text, &key)),
                    });
                }
                Err(error) => DecideError::Network(redact(&error, &key)),
            };
            if attempt == RETRIES {
                return Err(retry);
            }
            std::thread::sleep(self.backoff.saturating_mul(1 << attempt));
            attempt += 1;
        };
        let reply: Value = serde_json::from_str(&text)
            .map_err(|error| DecideError::Malformed(format!("not JSON: {error}")))?;
        criteria
            .iter()
            .map(|criterion| Ok((criterion.name.clone(), parse(&reply, &criterion.name)?)))
            .collect()
    }
}

/// `answers.<name>.noul` in `[0, 1]`.
fn parse(reply: &Value, name: &str) -> Result<f64, DecideError> {
    let p = reply["answers"][name]["noul"]
        .as_f64()
        .ok_or_else(|| DecideError::Malformed(format!("no number at `answers.{name}.noul`")))?;
    if p.is_finite() && (0.0..=1.0).contains(&p) {
        Ok(p)
    } else {
        Err(DecideError::Malformed(format!(
            "`{name}`: {p} is not in [0, 1]"
        )))
    }
}

fn redact(message: &str, key: &str) -> String {
    if key.is_empty() {
        message.to_owned()
    } else {
        message.replace(key, "***")
    }
}

fn cut(text: &str) -> String {
    let mut end = text.len().min(BODY_BYTES);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}
