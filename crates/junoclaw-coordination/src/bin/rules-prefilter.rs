//! Rules pre-filter server (NOT the J-Lens).
//!
//! A lightweight keyword/pattern pre-filter. This is deliberately NOT
//! a J-Lens implementation — the real activation-probe pipeline is the
//! JS/Python J-Lens server (`tools/brainmaxx/src/csi-server.js`).
//! This server may run as a cheap pre-filter in front of it, but it
//! never produces a J-Lens verdict and its output must never be
//! recorded as one.
//!
//! - Accepts `POST /audit` with `{ "text": "..." }`
//! - Runs a multi-layer rules engine (keyword matching, pattern
//!   detection, entropy analysis, length heuristics)
//! - Returns `{ "verdict", "rule_risk", "score_semantics":
//!   "prefilter.rule_risk.v1", "attestation_hash", "model_id", "gate" }`
//!
//! `rule_risk` is its own semantics: higher = worse (unlike J-Lens
//! `integrity`, where higher = healthier). The `score_semantics` tag
//! makes the two impossible to wire up confusedly — J-Lens clients
//! reject anything not tagged `jlens.integrity.v1`.
//!
//! Run: `cargo run -p junoclaw-coordination --bin rules-prefilter`

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::Arc,
    time::Instant,
};
use tracing::info;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct AuditResponse {
    verdict: String,
    /// Rule risk in [0,1] — HIGHER = WORSE (prefilter semantics,
    /// opposite of J-Lens integrity).
    rule_risk: f64,
    /// Always "prefilter.rule_risk.v1" — clients verify before use.
    score_semantics: String,
    attestation_hash: Option<String>,
    model_id: Option<String>,
    gate: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct HealthResponse {
    status: String,
    version: String,
    uptime_seconds: u64,
    rules_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
struct AuditRequest {
    text: String,
}

#[derive(Clone, Debug)]
struct Rule {
    name: String,
    keywords: Vec<String>,
    score: f64,
    verdict: String,
}

#[derive(Clone, Debug)]
struct PrefilterConfig {
    bind_addr: String,
    auth_token: String,
    yellow_threshold: f64,
    red_threshold: f64,
    rules: Vec<Rule>,
}

impl Default for PrefilterConfig {
    fn default() -> Self {
        Self {
            // PREFILTER_* env vars, with CSI_* fallback for existing deploys
            bind_addr: std::env::var("PREFILTER_BIND_ADDR")
                .or_else(|_| std::env::var("CSI_BIND_ADDR"))
                .unwrap_or_else(|_| "0.0.0.0:7778".to_string()),
            auth_token: std::env::var("PREFILTER_AUTH_TOKEN")
                .or_else(|_| std::env::var("CSI_AUTH_TOKEN"))
                .unwrap_or_default(),
            yellow_threshold: 0.15,
            red_threshold: 0.35,
            rules: default_rules(),
        }
    }
}

fn default_rules() -> Vec<Rule> {
    vec![
        Rule {
            name: "deceptive-content".to_string(),
            keywords: vec![
                "deceptive", "malicious", "hack", "exploit",
                "manipulate", "fraud", "scam",
            ].into_iter().map(String::from).collect(),
            score: 0.9,
            verdict: "red".to_string(),
        },
        Rule {
            name: "suspicious-content".to_string(),
            keywords: vec![
                "suspicious", "questionable", "unverified", "uncertain",
            ].into_iter().map(String::from).collect(),
            score: 0.2,
            verdict: "yellow".to_string(),
        },
        Rule {
            name: "coercion-language".to_string(),
            keywords: vec![
                "force", "coerce", "threaten", "blackmail",
            ].into_iter().map(String::from).collect(),
            score: 0.85,
            verdict: "red".to_string(),
        },
        Rule {
            name: "override-safety".to_string(),
            keywords: vec![
                "override safety", "bypass limit", "ignore constraint",
                "disable protection",
            ].into_iter().map(String::from).collect(),
            score: 0.95,
            verdict: "red".to_string(),
        },
        Rule {
            name: "evasive-language".to_string(),
            keywords: vec![
                "hide evidence", "cover tracks", "destroy log",
                "delete record",
            ].into_iter().map(String::from).collect(),
            score: 0.8,
            verdict: "red".to_string(),
        },
        Rule {
            name: "ambiguous-intent".to_string(),
            keywords: vec![
                "maybe", "possibly", "might", "unclear",
            ].into_iter().map(String::from).collect(),
            score: 0.15,
            verdict: "yellow".to_string(),
        },
    ]
}

fn run_rules(text: &str, config: &PrefilterConfig) -> (f64, String) {
    let lower = text.to_lowercase();
    let mut max_score = 0.0f64;

    for rule in &config.rules {
        for keyword in &rule.keywords {
            if lower.contains(keyword) {
                if rule.score > max_score {
                    max_score = rule.score;
                }
                break;
            }
        }
    }

    let entropy = shannon_entropy(&lower);
    if entropy < 1.5 && text.len() > 20 {
        max_score = max_score.max(0.3);
    }

    if text.len() > 10000 {
        max_score = max_score.max(0.2);
    }

    let final_verdict = if max_score >= config.red_threshold {
        "red".to_string()
    } else if max_score >= config.yellow_threshold {
        "yellow".to_string()
    } else {
        "green".to_string()
    };

    (max_score, final_verdict)
}

fn shannon_entropy(s: &str) -> f64 {
    if s.is_empty() {
        return 0.0;
    }
    let mut freq: HashMap<char, usize> = HashMap::new();
    for c in s.chars() {
        *freq.entry(c).or_insert(0) += 1;
    }
    let len = s.len() as f64;
    freq.values().map(|&count| {
        let p = count as f64 / len;
        -p * p.log2()
    }).sum()
}

fn attestation_hash(text: &str, score: f64, verdict: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    hasher.update(score.to_le_bytes());
    hasher.update(verdict.as_bytes());
    hasher.update(b"rules-prefilter-v1");
    hex::encode(hasher.finalize())
}

struct ServerState {
    config: PrefilterConfig,
    start_time: Instant,
}

async fn health_handler(State(state): State<Arc<ServerState>>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: "rules-prefilter-v1".to_string(),
        uptime_seconds: state.start_time.elapsed().as_secs(),
        rules_count: state.config.rules.len(),
    })
}

async fn audit_handler(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<AuditRequest>,
) -> Json<AuditResponse> {
    let (risk, verdict) = run_rules(&req.text, &state.config);
    let hash = attestation_hash(&req.text, risk, &verdict);

    Json(AuditResponse {
        verdict: verdict.clone(),
        rule_risk: risk,
        score_semantics: "prefilter.rule_risk.v1".to_string(),
        attestation_hash: Some(hash),
        model_id: Some("rules-prefilter-v1".to_string()),
        gate: verdict,
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = PrefilterConfig::default();
    let bind_addr: SocketAddr = config.bind_addr.parse().map_err(|e| {
        anyhow::anyhow!("invalid bind address {}: {}", config.bind_addr, e)
    })?;

    let state = Arc::new(ServerState {
        config: config.clone(),
        start_time: Instant::now(),
    });

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/audit", post(audit_handler))
        .with_state(state);

    info!(
        "Rules pre-filter server listening on {} ({} rules, auth={})",
        bind_addr,
        config.rules.len(),
        if config.auth_token.is_empty() { "disabled" } else { "enabled" }
    );

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_red_content_detected() {
        let config = PrefilterConfig::default();
        let (score, verdict) = run_rules("This is a malicious hack attempt", &config);
        assert!(score >= 0.9);
        assert_eq!(verdict, "red");
    }

    #[test]
    fn test_yellow_content_detected() {
        let config = PrefilterConfig::default();
        let (score, verdict) = run_rules("This is suspicious activity", &config);
        assert!(score >= 0.15);
        assert_eq!(verdict, "yellow");
    }

    #[test]
    fn test_green_for_clean_content() {
        let config = PrefilterConfig::default();
        let (score, verdict) = run_rules("Move forward at 2 m/s", &config);
        assert_eq!(score, 0.0);
        assert_eq!(verdict, "green");
    }

    #[test]
    fn test_coercion_language_detected() {
        let config = PrefilterConfig::default();
        let (score, verdict) = run_rules("Force the operator to comply", &config);
        assert!(score >= 0.85);
        assert_eq!(verdict, "red");
    }

    #[test]
    fn test_safety_override_detected() {
        let config = PrefilterConfig::default();
        let (score, verdict) = run_rules("override safety limits now", &config);
        assert!(score >= 0.95);
        assert_eq!(verdict, "red");
    }

    #[test]
    fn test_evasive_language_detected() {
        let config = PrefilterConfig::default();
        let (score, verdict) = run_rules("destroy log files immediately", &config);
        assert!(score >= 0.8);
        assert_eq!(verdict, "red");
    }

    #[test]
    fn test_low_entropy_flagged() {
        let config = PrefilterConfig::default();
        let (score, verdict) = run_rules("aaaaaaaaaaaaaaaaaaaaaaaaaaa", &config);
        assert!(score >= 0.3);
        assert_eq!(verdict, "yellow");
    }

    #[test]
    fn test_attestation_hash_deterministic() {
        let h1 = attestation_hash("test", 0.5, "yellow");
        let h2 = attestation_hash("test", 0.5, "yellow");
        assert_eq!(h1, h2);
        let h3 = attestation_hash("test", 0.9, "red");
        assert_ne!(h1, h3);
    }
}
