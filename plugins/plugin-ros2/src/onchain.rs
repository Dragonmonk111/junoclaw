use serde::Deserialize;

/// Lightweight CosmWasm smart-query client.
///
/// Queries on-chain contracts via the chain's LCD/RPC endpoint.
/// Used by the plugin to check on-chain SafetyEnvelope and CircuitBreaker
/// state instead of relying solely on in-memory state.
pub struct OnChainClient {
    rpc_url: String,
    safety_envelope_addr: String,
    circuit_breaker_addr: String,
    merkle_verifier_addr: String,
}

#[derive(Debug, Deserialize)]
struct AbciQueryResponse {
    result: AbciQueryResult,
}

#[derive(Debug, Deserialize)]
struct AbciQueryResult {
    response: AbciQueryData,
}

#[derive(Debug, Deserialize)]
struct AbciQueryData {
    value: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct IsLockedResponse {
    pub robot_id: String,
    pub is_locked: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct GetEnvelopeResponse {
    pub robot_id: String,
    pub params: EnvelopeParams,
    pub version: u32,
    pub updated_at: u64,
    pub updated_by: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
pub struct EnvelopeParams {
    pub max_speed_milli: u64,
    pub max_force_milli: u64,
    pub min_collision_distance_milli: u64,
    pub max_tilt_milli_degrees: u64,
    pub max_acceleration_milli: u64,
    pub human_proximity_allowed: bool,
    #[serde(default)]
    pub max_arm_force_milli: u64,
    #[serde(default)]
    pub max_joint_torque_milli: u64,
}

impl OnChainClient {
    pub fn new(
        rpc_url: String,
        safety_envelope_addr: String,
        circuit_breaker_addr: String,
        merkle_verifier_addr: String,
    ) -> Self {
        Self {
            rpc_url,
            safety_envelope_addr,
            circuit_breaker_addr,
            merkle_verifier_addr,
        }
    }

    /// Query the circuit breaker contract: is this robot's intent-tier locked?
    pub async fn is_locked(&self, robot_id: &str) -> anyhow::Result<IsLockedResponse> {
        let query_msg = serde_json::json!({
            "is_locked": { "robot_id": robot_id }
        });
        let resp = self
            .smart_query(&self.circuit_breaker_addr, &query_msg)
            .await?;
        let result: IsLockedResponse = serde_json::from_slice(&resp)?;
        Ok(result)
    }

    /// Query the safety envelope contract for the robot's current envelope.
    pub async fn get_envelope(&self, robot_id: &str) -> anyhow::Result<GetEnvelopeResponse> {
        let query_msg = serde_json::json!({
            "get_envelope": { "robot_id": robot_id }
        });
        let resp = self
            .smart_query(&self.safety_envelope_addr, &query_msg)
            .await?;
        let result: GetEnvelopeResponse = serde_json::from_slice(&resp)?;
        Ok(result)
    }

    /// Query the merkle verifier for an anchored batch root.
    pub async fn get_root(
        &self,
        robot_id: &str,
        batch_height: u64,
    ) -> anyhow::Result<Vec<u8>> {
        let query_msg = serde_json::json!({
            "get_root": { "robot_id": robot_id, "batch_height": batch_height }
        });
        self.smart_query(&self.merkle_verifier_addr, &query_msg)
            .await
    }

    /// Low-level CosmWasm smart query via ABCI.
    ///
    /// Uses the `/abci_query` endpoint with the standard CosmWasm query path:
    /// `/cosmwasm.wasm.v1.Query/SmartContractState/{contract_addr}/{query_base64}`
    async fn smart_query(
        &self,
        contract_addr: &str,
        query_msg: &serde_json::Value,
    ) -> anyhow::Result<Vec<u8>> {
        let query_bytes = serde_json::to_vec(query_msg)?;
        let query_b64 = base64_encode(&query_bytes);

        let url = format!(
            "{}/abci_query?path=\"/cosmwasm.wasm.v1.Query/SmartContractState/{}%2F{}\"",
            self.rpc_url.trim_end_matches('/'),
            contract_addr,
            query_b64
        );

        let client = reqwest::Client::new();
        let resp = client.get(&url).send().await?;
        let body: AbciQueryResponse = resp.json().await?;
        let decoded = base64_decode(&body.result.response.value)?;
        Ok(decoded)
    }
}

/// Minimal base64 encoder (URL-safe, no padding) for CosmWasm queries.
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        }
    }
    result
}

/// Minimal base64 decoder (standard alphabet with padding).
fn base64_decode(s: &str) -> anyhow::Result<Vec<u8>> {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let s = s.trim_end_matches('=');
    let mut result = Vec::new();
    let bytes = s.as_bytes();

    for chunk in bytes.chunks(4) {
        let mut vals = [0u32; 4];
        for (i, &b) in chunk.iter().enumerate() {
            vals[i] = CHARS.iter().position(|&c| c == b).ok_or_else(|| {
                anyhow::anyhow!("invalid base64 character: {}", b as char)
            })? as u32;
        }
        let quad = (vals[0] << 18) | (vals[1] << 12) | (vals[2] << 6) | vals[3];
        result.push((quad >> 16) as u8);
        if chunk.len() > 2 {
            result.push((quad >> 8) as u8);
        }
        if chunk.len() > 3 {
            result.push(quad as u8);
        }
    }
    Ok(result)
}
