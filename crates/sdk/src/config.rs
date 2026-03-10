use open_zk_core::types::{SecurityLevel, ZkvmBackend};
use open_zk_orchestrator::IntentResolver;
use std::time::Duration;

/// Top-level SDK configuration built from user intent.
#[derive(Debug, Clone)]
pub struct OpenZkConfig {
    pub backend: ZkvmBackend,
    pub allowed_backends: Vec<ZkvmBackend>,
    pub target_finality: Duration,
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
        IntentResolver::resolve(
            self.backend,
            &self.allowed_backends,
            self.target_finality,
            self.security,
        )
    }
}

/// Builder for constructing an `OpenZkConfig`.
#[derive(Debug, Default)]
pub struct OpenZkConfigBuilder {
    backend: Option<ZkvmBackend>,
    allowed_backends: Option<Vec<ZkvmBackend>>,
    target_finality: Option<Duration>,
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
    pub fn backend(mut self, backend: ZkvmBackend) -> Self {
        self.backend = Some(backend);
        self
    }

    pub fn allowed_backends(mut self, backends: Vec<ZkvmBackend>) -> Self {
        self.allowed_backends = Some(backends);
        self
    }

    pub fn target_finality(mut self, d: Duration) -> Self {
        self.target_finality = Some(d);
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
            backend: self.backend.unwrap_or(ZkvmBackend::Auto),
            allowed_backends: self
                .allowed_backends
                .unwrap_or_else(|| IntentResolver::DEFAULT_ALLOWED_BACKENDS.to_vec()),
            target_finality: self.target_finality.unwrap_or(Duration::from_secs(30 * 60)),
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
    fn build_config_auto_backend_defaults() {
        let config = OpenZkConfig::builder()
            .target_finality(Duration::from_secs(600))
            .security(SecurityLevel::Standard)
            .l1_rpc_url("http://localhost:8545")
            .l2_rpc_url("http://localhost:9545")
            .l1_beacon_url("http://localhost:5052")
            .build()
            .unwrap();

        assert_eq!(config.backend, ZkvmBackend::Auto);
        assert_eq!(
            config.allowed_backends,
            vec![ZkvmBackend::Sp1, ZkvmBackend::RiscZero]
        );
        let resolved = config.resolve();
        // Standard defaults to Sentinel
        assert_eq!(resolved.proof_mode, ProofMode::Sentinel);
        assert_eq!(resolved.backend, ZkvmBackend::Sp1);
    }

    #[test]
    fn auto_with_custom_allowlist() {
        let config = OpenZkConfig::builder()
            .allowed_backends(vec![ZkvmBackend::RiscZero])
            .security(SecurityLevel::Standard)
            .l1_rpc_url("http://localhost:8545")
            .l2_rpc_url("http://localhost:9545")
            .l1_beacon_url("http://localhost:5052")
            .build()
            .unwrap();

        let resolved = config.resolve();
        assert_eq!(resolved.backend, ZkvmBackend::RiscZero);
    }

    #[test]
    fn explicit_backend_ignores_allowlist() {
        let config = OpenZkConfig::builder()
            .backend(ZkvmBackend::Sp1)
            .allowed_backends(vec![ZkvmBackend::RiscZero])
            .security(SecurityLevel::Standard)
            .l1_rpc_url("http://localhost:8545")
            .l2_rpc_url("http://localhost:9545")
            .l1_beacon_url("http://localhost:5052")
            .build()
            .unwrap();

        let resolved = config.resolve();
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
