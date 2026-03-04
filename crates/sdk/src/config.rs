use open_zk_core::types::SecurityLevel;
use open_zk_orchestrator::IntentResolver;
use std::time::Duration;

/// Top-level SDK configuration built from user intent.
#[derive(Debug, Clone)]
pub struct OpenZkConfig {
    pub target_finality: Duration,
    pub max_cost_per_proof: f64,
    pub security: SecurityLevel,
    pub l1_rpc_url: String,
    pub l2_rpc_url: String,
    pub l1_beacon_url: String,
}

impl OpenZkConfig {
    pub fn builder() -> OpenZkConfigBuilder {
        OpenZkConfigBuilder::default()
    }

    /// Resolve this config into concrete proving parameters.
    pub fn resolve(&self) -> open_zk_orchestrator::ResolvedIntent {
        IntentResolver::resolve(self.target_finality, self.max_cost_per_proof, self.security)
    }
}

/// Builder for constructing an `OpenZkConfig`.
#[derive(Debug, Default)]
pub struct OpenZkConfigBuilder {
    target_finality: Option<Duration>,
    max_cost_per_proof: Option<f64>,
    security: Option<SecurityLevel>,
    l1_rpc_url: Option<String>,
    l2_rpc_url: Option<String>,
    l1_beacon_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("missing required field: {0}")]
    MissingField(&'static str),
}

impl OpenZkConfigBuilder {
    pub fn target_finality(mut self, d: Duration) -> Self {
        self.target_finality = Some(d);
        self
    }

    pub fn max_cost_per_proof(mut self, cost: f64) -> Self {
        self.max_cost_per_proof = Some(cost);
        self
    }

    pub fn security(mut self, level: SecurityLevel) -> Self {
        self.security = Some(level);
        self
    }

    pub fn l1_rpc_url(mut self, url: impl Into<String>) -> Self {
        self.l1_rpc_url = Some(url.into());
        self
    }

    pub fn l2_rpc_url(mut self, url: impl Into<String>) -> Self {
        self.l2_rpc_url = Some(url.into());
        self
    }

    pub fn l1_beacon_url(mut self, url: impl Into<String>) -> Self {
        self.l1_beacon_url = Some(url.into());
        self
    }

    pub fn build(self) -> Result<OpenZkConfig, ConfigError> {
        Ok(OpenZkConfig {
            target_finality: self
                .target_finality
                .unwrap_or(Duration::from_secs(30 * 60)),
            max_cost_per_proof: self.max_cost_per_proof.unwrap_or(1.0),
            security: self.security.unwrap_or(SecurityLevel::Standard),
            l1_rpc_url: self
                .l1_rpc_url
                .ok_or(ConfigError::MissingField("l1_rpc_url"))?,
            l2_rpc_url: self
                .l2_rpc_url
                .ok_or(ConfigError::MissingField("l2_rpc_url"))?,
            l1_beacon_url: self
                .l1_beacon_url
                .ok_or(ConfigError::MissingField("l1_beacon_url"))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_zk_core::types::{ProofMode, ZkvmBackend};

    #[test]
    fn build_config_and_resolve() {
        let config = OpenZkConfig::builder()
            .target_finality(Duration::from_secs(600))
            .max_cost_per_proof(0.50)
            .security(SecurityLevel::Standard)
            .l1_rpc_url("http://localhost:8545")
            .l2_rpc_url("http://localhost:9545")
            .l1_beacon_url("http://localhost:5052")
            .build()
            .unwrap();

        let resolved = config.resolve();
        assert_eq!(resolved.proof_mode, ProofMode::Beacon);
        assert_eq!(resolved.backend, ZkvmBackend::Sp1);
    }

    #[test]
    fn missing_rpc_url_errors() {
        let result = OpenZkConfig::builder()
            .l2_rpc_url("http://localhost:9545")
            .l1_beacon_url("http://localhost:5052")
            .build();
        assert!(result.is_err());
    }
}
