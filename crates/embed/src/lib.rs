//! OpenAI-compatible embeddings client.
//!
//! Posts `{base_url}/embeddings` with `{ "model", "input": [...] }` and reads back
//! `{ "data": [{ "embedding": [...] }, ...] }`. Defaults (via `mdm-config`) to OpenRouter
//! (`https://openrouter.ai/api/v1`) but works with any OpenAI-compatible provider — the
//! base URL, key, model, and dimensions all come from env. Nothing is hardcoded to a
//! provider beyond the default base URL.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EmbedError {
    #[error("embedding request failed: {0}")]
    Transport(String),
    #[error("embedding provider returned {status}: {body}")]
    Provider { status: u16, body: String },
    #[error("unexpected embedding response: {0}")]
    Parse(String),
    #[error(
        "dimension mismatch: configured {expected}, model returned {got} (set MDM_EMBEDDING_DIMENSIONS to match the model)"
    )]
    Dimension { expected: usize, got: usize },
}

/// An embeddings client for one configured model.
#[derive(Clone)]
pub struct Embedder {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    dimensions: usize,
    referer: Option<String>,
    title: Option<String>,
}

#[derive(Serialize)]
struct EmbeddingRequest<'a> {
    model: &'a str,
    input: &'a [String],
}

#[derive(Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingDatum>,
}

#[derive(Deserialize)]
struct EmbeddingDatum {
    embedding: Vec<f32>,
    #[serde(default)]
    index: usize,
}

impl Embedder {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        model: impl Into<String>,
        dimensions: usize,
        timeout: Duration,
        referer: Option<String>,
        title: Option<String>,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            api_key: api_key.into(),
            model: model.into(),
            dimensions,
            referer,
            title,
        }
    }

    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Embed a batch of inputs; returns one vector per input, in input order.
    pub async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let mut req = self
            .http
            .post(format!("{}/embeddings", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&EmbeddingRequest {
                model: &self.model,
                input: inputs,
            });
        // OpenRouter attribution headers (optional).
        if let Some(referer) = &self.referer {
            req = req.header("HTTP-Referer", referer);
        }
        if let Some(title) = &self.title {
            req = req.header("X-Title", title);
        }

        let resp = req
            .send()
            .await
            .map_err(|e| EmbedError::Transport(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| EmbedError::Transport(e.to_string()))?;
        if !status.is_success() {
            return Err(EmbedError::Provider {
                status: status.as_u16(),
                body: truncate(&text, 300),
            });
        }
        parse_embeddings(&text, inputs.len(), self.dimensions)
    }
}

fn parse_embeddings(
    text: &str,
    expected_count: usize,
    dims: usize,
) -> Result<Vec<Vec<f32>>, EmbedError> {
    let parsed: EmbeddingResponse =
        serde_json::from_str(text).map_err(|e| EmbedError::Parse(e.to_string()))?;
    if parsed.data.len() != expected_count {
        return Err(EmbedError::Parse(format!(
            "expected {expected_count} embeddings, got {}",
            parsed.data.len()
        )));
    }
    let mut data = parsed.data;
    data.sort_by_key(|d| d.index);
    let mut out = Vec::with_capacity(data.len());
    for d in data {
        if d.embedding.len() != dims {
            return Err(EmbedError::Dimension {
                expected: dims,
                got: d.embedding.len(),
            });
        }
        out.push(d.embedding);
    }
    Ok(out)
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}…", &s[..n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_openai_compatible_response() {
        let body = r#"{"object":"list","data":[
            {"object":"embedding","index":1,"embedding":[0.4,0.5,0.6]},
            {"object":"embedding","index":0,"embedding":[0.1,0.2,0.3]}
        ],"model":"x","usage":{"prompt_tokens":4,"total_tokens":4}}"#;
        let out = parse_embeddings(body, 2, 3).expect("parse");
        // re-ordered by index
        assert_eq!(out[0], vec![0.1, 0.2, 0.3]);
        assert_eq!(out[1], vec![0.4, 0.5, 0.6]);
    }

    #[test]
    fn rejects_dimension_mismatch() {
        let body = r#"{"data":[{"index":0,"embedding":[0.1,0.2]}]}"#;
        assert!(matches!(
            parse_embeddings(body, 1, 3),
            Err(EmbedError::Dimension {
                expected: 3,
                got: 2
            })
        ));
    }

    #[test]
    fn rejects_wrong_count() {
        let body = r#"{"data":[{"index":0,"embedding":[0.1,0.2,0.3]}]}"#;
        assert!(matches!(
            parse_embeddings(body, 2, 3),
            Err(EmbedError::Parse(_))
        ));
    }
}
