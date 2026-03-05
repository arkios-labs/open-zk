//! TOML configuration file support.

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Top-level TOML configuration file structure.
///
/// Example `open-zk.toml`:
/// ```toml
/// [network]
/// l1_rpc_url = "http://localhost:8545"
/// l2_rpc_url = "http://localhost:9545"
/// l1_beacon_url = "http://localhost:5052"
///
/// [proving]
/// backend = "sp1"
/// mode = "beacon"
/// security = "standard"
/// target_finality_secs = 1800
/// max_cost_per_proof = 1.0
/// max_concurrent_proofs = 4
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub network: NetworkConfig,
    #[serde(default)]
    pub proving: ProvingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub l1_rpc_url: String,
    pub l2_rpc_url: String,
    pub l1_beacon_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProvingConfig {
    #[serde(default = "default_backend")]
    pub backend: String,
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default = "default_security")]
    pub security: String,
    #[serde(default = "default_target_finality")]
    pub target_finality_secs: u64,
    #[serde(default = "default_max_cost")]
    pub max_cost_per_proof: f64,
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent_proofs: usize,
}

fn default_backend() -> String {
    "sp1".to_string()
}
fn default_mode() -> String {
    "beacon".to_string()
}
fn default_security() -> String {
    "standard".to_string()
}
fn default_target_finality() -> u64 {
    1800
}
fn default_max_cost() -> f64 {
    1.0
}
fn default_max_concurrent() -> usize {
    4
}

impl Default for ProvingConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            mode: default_mode(),
            security: default_security(),
            target_finality_secs: default_target_finality(),
            max_cost_per_proof: default_max_cost(),
            max_concurrent_proofs: default_max_concurrent(),
        }
    }
}

impl CliConfig {
    /// Load a config from a TOML file.
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Convert to an `OpenZkConfig` for the SDK.
    pub fn to_sdk_config(&self) -> anyhow::Result<open_zk::OpenZkConfig> {
        let security = match self.proving.security.as_str() {
            "maximum" => open_zk_core::types::SecurityLevel::Maximum,
            "economy" => open_zk_core::types::SecurityLevel::Economy,
            _ => open_zk_core::types::SecurityLevel::Standard,
        };

        open_zk::OpenZkConfig::builder()
            .target_finality(std::time::Duration::from_secs(
                self.proving.target_finality_secs,
            ))
            .max_cost_per_proof(self.proving.max_cost_per_proof)
            .security(security)
            .l1_rpc_url(&self.network.l1_rpc_url)
            .l2_rpc_url(&self.network.l2_rpc_url)
            .l1_beacon_url(&self.network.l1_beacon_url)
            .build()
            .map_err(|e| anyhow::anyhow!("config error: {}", e))
    }

    /// Check if mock proving mode is active.
    ///
    /// Mock mode is enabled when:
    /// - `backend = "mock"` in the config, OR
    /// - `SP1_PROVER=mock` environment variable is set
    ///
    /// In mock mode, proofs are generated instantly without real ZK computation.
    /// This is useful for devnet testing and CI pipelines.
    pub fn is_mock_mode(&self) -> bool {
        self.proving.backend == "mock" || std::env::var("SP1_PROVER").is_ok_and(|v| v == "mock")
    }

    /// Generate a default TOML config string.
    pub fn default_toml() -> String {
        let config = Self {
            network: NetworkConfig {
                l1_rpc_url: "http://localhost:8545".to_string(),
                l2_rpc_url: "http://localhost:9545".to_string(),
                l1_beacon_url: "http://localhost:5052".to_string(),
            },
            proving: ProvingConfig::default(),
        };
        toml::to_string_pretty(&config).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_toml_config() {
        let toml_str = r#"
[network]
l1_rpc_url = "http://localhost:8545"
l2_rpc_url = "http://localhost:9545"
l1_beacon_url = "http://localhost:5052"

[proving]
backend = "sp1"
mode = "beacon"
security = "standard"
target_finality_secs = 1800
max_cost_per_proof = 1.0
max_concurrent_proofs = 4
"#;
        let config: CliConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.network.l1_rpc_url, "http://localhost:8545");
        assert_eq!(config.proving.backend, "sp1");
        assert_eq!(config.proving.max_concurrent_proofs, 4);
    }

    #[test]
    fn config_to_sdk_config() {
        let config = CliConfig {
            network: NetworkConfig {
                l1_rpc_url: "http://l1:8545".to_string(),
                l2_rpc_url: "http://l2:9545".to_string(),
                l1_beacon_url: "http://beacon:5052".to_string(),
            },
            proving: ProvingConfig::default(),
        };

        let sdk_config = config.to_sdk_config().unwrap();
        let resolved = sdk_config.resolve();
        assert_eq!(resolved.proof_mode, open_zk_core::types::ProofMode::Beacon);
    }

    #[test]
    fn default_toml_roundtrip() {
        let toml_str = CliConfig::default_toml();
        let parsed: CliConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(parsed.proving.backend, "sp1");
    }

    #[test]
    fn mock_mode_from_backend_config() {
        let config = CliConfig {
            network: NetworkConfig {
                l1_rpc_url: "http://l1:8545".to_string(),
                l2_rpc_url: "http://l2:9545".to_string(),
                l1_beacon_url: "http://beacon:5052".to_string(),
            },
            proving: ProvingConfig {
                backend: "mock".to_string(),
                ..ProvingConfig::default()
            },
        };
        assert!(config.is_mock_mode());
    }

    #[test]
    fn non_mock_mode_by_default() {
        let config = CliConfig {
            network: NetworkConfig {
                l1_rpc_url: "http://l1:8545".to_string(),
                l2_rpc_url: "http://l2:9545".to_string(),
                l1_beacon_url: "http://beacon:5052".to_string(),
            },
            proving: ProvingConfig::default(),
        };
        // Only true if SP1_PROVER=mock env var is NOT set
        // (we can't control env in this test, so just verify config-based detection)
        assert!(!config.is_mock_mode() || std::env::var("SP1_PROVER").is_ok_and(|v| v == "mock"));
    }
}
