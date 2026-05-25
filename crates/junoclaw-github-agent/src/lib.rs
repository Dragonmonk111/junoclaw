//! GitHub App authentication for JunoClaw agents.
//!
//! A JunoClaw agent holds two independent key pairs:
//!   - **Cosmos secp256k1 key** — on-chain identity, agent-registry membership, task settlement
//!   - **GitHub App RSA key** — off-chain PR authorship, appears as `YourApp[bot]`
//!
//! The two are completely independent. On-chain reputation does not cross-contaminate
//! with GitHub access. The App authenticates under the DA0-DA0 (or CosmosContracts) org
//! so it's institutionally owned, not personal-account-dependent.
//!
//! # Setup (one-time, ~5 minutes)
//!
//! 1. Create the App: `https://github.com/organizations/DA0-DA0/settings/apps/new`
//! 2. Permissions required:
//!    - `Contents: Read & Write`
//!    - `Pull requests: Read & Write`
//!    - `Metadata: Read` (required baseline)
//!    - `Workflows: Read & Write` (optional — only if agent needs to trigger CI)
//! 3. Generate a private key (RSA PEM) — download and store as `GITHUB_APP_PRIVATE_KEY` env var
//! 4. Note the App ID (`GITHUB_APP_ID`) and Installation ID (`GITHUB_APP_INSTALLATION_ID`)
//!    (Installation ID: `https://github.com/organizations/DA0-DA0/settings/installations`)
//!
//! # Usage
//!
//! ```rust,no_run
//! use junoclaw_github_agent::{GitHubAppAuth, PullRequestDraft};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let auth = GitHubAppAuth::from_env()?;
//!     let token = auth.installation_token().await?;
//!
//!     let pr = PullRequestDraft {
//!         owner: "CosmosContracts".into(),
//!         repo: "juno-network-skill".into(),
//!         title: "docs(references): add references/junoclaw.md".into(),
//!         head: "Dragonmonk111:feat/junoclaw-reference".into(),
//!         base: "main".into(),
//!         body: "...".into(),
//!         maintainer_can_modify: true,
//!     };
//!
//!     let url = token.open_pull_request(pr).await?;
//!     println!("PR opened: {url}");
//!     Ok(())
//! }
//! ```

use anyhow::{Context, Result};
use chrono::Utc;
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::{Deserialize, Serialize};

const GITHUB_API: &str = "https://api.github.com";

// ---------------------------------------------------------------------------
// JWT claims for GitHub App authentication
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct AppClaims {
    iat: i64,
    exp: i64,
    iss: u64,
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// GitHub App credentials. Load once; call [`installation_token()`] per operation
/// (tokens are valid for 1 hour, so cache as needed).
pub struct GitHubAppAuth {
    app_id: u64,
    encoding_key: EncodingKey,
    installation_id: u64,
    client: reqwest::Client,
}

/// A short-lived installation access token (valid ~1 hour).
/// Acts as a Bot, not a User — GitHub TOS treats these categorically differently.
pub struct InstallationToken {
    token: String,
    client: reqwest::Client,
}

/// Pull request to open on behalf of the agent.
pub struct PullRequestDraft {
    pub owner: String,
    pub repo: String,
    pub title: String,
    pub head: String,
    pub base: String,
    pub body: String,
    pub maintainer_can_modify: bool,
}

#[derive(Deserialize)]
struct InstallationTokenResponse {
    token: String,
}

#[derive(Serialize)]
struct CreatePrPayload<'a> {
    title: &'a str,
    head: &'a str,
    base: &'a str,
    body: &'a str,
    maintainer_can_modify: bool,
}

#[derive(Deserialize)]
struct PrResponse {
    html_url: String,
    number: u64,
}

#[derive(Deserialize)]
struct CommitResponse {
    sha: String,
}

// ---------------------------------------------------------------------------
// Implementation
// ---------------------------------------------------------------------------

impl GitHubAppAuth {
    /// Load credentials from environment variables:
    /// - `GITHUB_APP_ID`
    /// - `GITHUB_APP_PRIVATE_KEY` (PEM, newlines as `\n`)
    /// - `GITHUB_APP_INSTALLATION_ID`
    pub fn from_env() -> Result<Self> {
        let app_id: u64 = std::env::var("GITHUB_APP_ID")
            .context("GITHUB_APP_ID not set")?
            .parse()
            .context("GITHUB_APP_ID must be a number")?;

        let pem = std::env::var("GITHUB_APP_PRIVATE_KEY")
            .context("GITHUB_APP_PRIVATE_KEY not set")?
            .replace("\\n", "\n");

        let installation_id: u64 = std::env::var("GITHUB_APP_INSTALLATION_ID")
            .context("GITHUB_APP_INSTALLATION_ID not set")?
            .parse()
            .context("GITHUB_APP_INSTALLATION_ID must be a number")?;

        let encoding_key = EncodingKey::from_rsa_pem(pem.as_bytes())
            .context("Failed to parse GITHUB_APP_PRIVATE_KEY as RSA PEM")?;

        let client = reqwest::Client::builder()
            .user_agent("junoclaw-github-agent/0.1.0")
            .build()?;

        Ok(Self { app_id, encoding_key, installation_id, client })
    }

    /// Generate a signed JWT (valid 9 minutes) and exchange it for an
    /// installation access token. The token acts as a Bot, not a User.
    pub async fn installation_token(&self) -> Result<InstallationToken> {
        let now = Utc::now().timestamp();
        let claims = AppClaims {
            iat: now - 60,  // 60s clock skew buffer
            exp: now + 540, // 9 minutes (GitHub max is 10)
            iss: self.app_id,
        };

        let app_jwt = encode(&Header::new(Algorithm::RS256), &claims, &self.encoding_key)
            .context("Failed to sign GitHub App JWT")?;

        let url = format!("{GITHUB_API}/app/installations/{}/access_tokens", self.installation_id);
        let resp: InstallationTokenResponse = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {app_jwt}"))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?
            .error_for_status()
            .context("Failed to get installation access token")?
            .json()
            .await?;

        tracing::debug!("GitHub App installation token obtained for installation {}", self.installation_id);

        Ok(InstallationToken { token: resp.token, client: self.client.clone() })
    }
}

impl InstallationToken {
    fn auth_header(&self) -> String {
        format!("Bearer {}", self.token)
    }

    /// Open a pull request. Returns the PR URL and number.
    pub async fn open_pull_request(&self, pr: PullRequestDraft) -> Result<(String, u64)> {
        let url = format!("{GITHUB_API}/repos/{}/{}/pulls", pr.owner, pr.repo);
        let payload = CreatePrPayload {
            title: &pr.title,
            head: &pr.head,
            base: &pr.base,
            body: &pr.body,
            maintainer_can_modify: pr.maintainer_can_modify,
        };

        let resp: PrResponse = self.client
            .post(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/vnd.github+json")
            .json(&payload)
            .send()
            .await?
            .error_for_status()
            .context("Failed to open pull request")?
            .json()
            .await?;

        tracing::info!("PR #{} opened: {}", resp.number, resp.html_url);
        Ok((resp.html_url, resp.number))
    }

    /// Push a file to a branch (create or update).
    /// `content` is the raw file content (will be base64-encoded).
    pub async fn push_file(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        path: &str,
        content: &str,
        commit_message: &str,
    ) -> Result<String> {
        let url = format!("{GITHUB_API}/repos/{owner}/{repo}/contents/{path}");

        // Get existing SHA if the file exists (needed for updates)
        let existing_sha: Option<String> = self.client
            .get(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/vnd.github+json")
            .query(&[("ref", branch)])
            .send()
            .await?
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| v.get("sha")?.as_str().map(String::from));

        let encoded = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            content.as_bytes(),
        );

        let mut payload = serde_json::json!({
            "message": commit_message,
            "content": encoded,
            "branch": branch,
        });
        if let Some(sha) = existing_sha {
            payload["sha"] = serde_json::Value::String(sha);
        }

        let resp: CommitResponse = self.client
            .put(&url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/vnd.github+json")
            .json(&payload)
            .send()
            .await?
            .error_for_status()
            .context("Failed to push file")?
            .json::<serde_json::Value>()
            .await?
            .get("commit")
            .and_then(|c| c.get("sha"))
            .and_then(|s| s.as_str())
            .map(|s| CommitResponse { sha: s.to_string() })
            .context("Missing commit SHA in response")?;

        tracing::info!("Committed {path} on {branch}: {}", resp.sha);
        Ok(resp.sha)
    }

    /// Create a branch from a base ref (usually "main").
    pub async fn create_branch(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        from_ref: &str,
    ) -> Result<()> {
        // Resolve the SHA of from_ref
        let ref_url = format!("{GITHUB_API}/repos/{owner}/{repo}/git/ref/heads/{from_ref}");
        let base_sha: String = self.client
            .get(&ref_url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/vnd.github+json")
            .send()
            .await?
            .error_for_status()
            .context("Failed to resolve base ref")?
            .json::<serde_json::Value>()
            .await?
            .pointer("/object/sha")
            .and_then(|s| s.as_str())
            .map(String::from)
            .context("Missing SHA in ref response")?;

        let create_url = format!("{GITHUB_API}/repos/{owner}/{repo}/git/refs");
        self.client
            .post(&create_url)
            .header("Authorization", self.auth_header())
            .header("Accept", "application/vnd.github+json")
            .json(&serde_json::json!({
                "ref": format!("refs/heads/{branch}"),
                "sha": base_sha,
            }))
            .send()
            .await?
            .error_for_status()
            .context("Failed to create branch")?;

        tracing::info!("Branch {branch} created from {from_ref} @ {base_sha}");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum AgentGitHubError {
    #[error("JWT signing failed: {0}")]
    JwtSign(#[from] jsonwebtoken::errors::Error),
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("GitHub API error: {message}")]
    Api { message: String },
}
