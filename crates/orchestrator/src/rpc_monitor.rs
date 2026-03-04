//! RPC-based chain monitor for live L1/L2 monitoring.
//!
//! Connects to L1 and L2 execution clients to track the latest proven block,
//! safe block, and finalized L1 head.

#[cfg(not(feature = "rpc"))]
mod skeleton {
    use crate::engine::DisputeInfo;
    use crate::monitor::{ChainMonitor, ChainState};
    use async_trait::async_trait;

    /// RPC-based chain monitor (skeleton — enable `rpc` feature for full impl).
    pub struct RpcChainMonitor {
        pub l1_rpc_url: String,
        pub l2_rpc_url: String,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum RpcMonitorError {
        #[error("L1 RPC error: {0}")]
        L1Rpc(String),
        #[error("L2 RPC error: {0}")]
        L2Rpc(String),
    }

    impl RpcChainMonitor {
        pub fn new(l1_rpc_url: String, l2_rpc_url: String) -> Self {
            Self {
                l1_rpc_url,
                l2_rpc_url,
            }
        }
    }

    #[async_trait]
    impl ChainMonitor for RpcChainMonitor {
        type Error = RpcMonitorError;

        async fn get_state(&self) -> Result<ChainState, Self::Error> {
            Err(RpcMonitorError::L1Rpc(
                "RPC chain monitor not available — enable `rpc` feature".to_string(),
            ))
        }

        async fn active_dispute(&self) -> Option<DisputeInfo> {
            None
        }
    }
}

#[cfg(feature = "rpc")]
mod full {
    use crate::engine::DisputeInfo;
    use crate::monitor::{ChainMonitor, ChainState};
    use alloy_primitives::Address;
    use alloy_provider::{Provider, ProviderBuilder};
    use alloy_rpc_types_eth::BlockNumberOrTag;
    use alloy_sol_types::SolCall;
    use async_trait::async_trait;
    use open_zk_contracts::abi::IOpenZkL2OutputOracle;
    use std::time::SystemTime;
    use tracing::{debug, warn};
    use url::Url;

    /// RPC-based chain monitor that fetches live state from L1/L2 nodes.
    pub struct RpcChainMonitor {
        l1_rpc_url: Url,
        l2_rpc_url: Url,
        /// Address of the L2OutputOracle contract on L1.
        oracle_address: Address,
        /// Address of the DisputeGame contract on L1 (optional).
        dispute_address: Option<Address>,
    }

    #[derive(Debug, thiserror::Error)]
    pub enum RpcMonitorError {
        #[error("L1 RPC error: {0}")]
        L1Rpc(String),
        #[error("L2 RPC error: {0}")]
        L2Rpc(String),
        #[error("contract call error: {0}")]
        Contract(String),
        #[error("URL parse error: {0}")]
        UrlParse(#[from] url::ParseError),
    }

    impl RpcChainMonitor {
        /// Create a new RPC chain monitor.
        ///
        /// # Arguments
        /// - `l1_rpc_url`: L1 execution client URL
        /// - `l2_rpc_url`: L2 execution client URL
        /// - `oracle_address`: Address of IOpenZkL2OutputOracle on L1
        pub fn new(
            l1_rpc_url: &str,
            l2_rpc_url: &str,
            oracle_address: Address,
        ) -> Result<Self, RpcMonitorError> {
            Ok(Self {
                l1_rpc_url: l1_rpc_url.parse()?,
                l2_rpc_url: l2_rpc_url.parse()?,
                oracle_address,
                dispute_address: None,
            })
        }

        /// Set the dispute game contract address.
        pub fn with_dispute_address(mut self, address: Address) -> Self {
            self.dispute_address = Some(address);
            self
        }
    }

    #[async_trait]
    impl ChainMonitor for RpcChainMonitor {
        type Error = RpcMonitorError;

        async fn get_state(&self) -> Result<ChainState, Self::Error> {
            // Create providers
            let l1_provider = ProviderBuilder::new()
                .connect_http(self.l1_rpc_url.clone());
            let l2_provider = ProviderBuilder::new()
                .connect_http(self.l2_rpc_url.clone());

            // 1. Fetch latest finalized L1 block
            let l1_block = l1_provider
                .get_block_by_number(BlockNumberOrTag::Finalized)
                .await
                .map_err(|e| RpcMonitorError::L1Rpc(e.to_string()))?
                .ok_or_else(|| {
                    RpcMonitorError::L1Rpc("finalized block not found".to_string())
                })?;

            let l1_head = l1_block.header.hash;
            let l1_block_number = l1_block.header.number;
            debug!(l1_head = %l1_head, l1_block_number, "fetched L1 finalized block");

            // 2. Fetch latest safe L2 block
            let l2_block = l2_provider
                .get_block_by_number(BlockNumberOrTag::Safe)
                .await
                .map_err(|e| RpcMonitorError::L2Rpc(e.to_string()))?
                .ok_or_else(|| {
                    RpcMonitorError::L2Rpc("safe block not found".to_string())
                })?;

            let l2_safe_block = l2_block.header.number;
            debug!(l2_safe_block, "fetched L2 safe block");

            // 3. Fetch latest proven L2 block from L2OutputOracle contract
            let call_data =
                IOpenZkL2OutputOracle::latestBlockNumberCall {}.abi_encode();
            let call_result = l1_provider
                .call(
                    alloy_rpc_types_eth::TransactionRequest::default()
                        .to(self.oracle_address)
                        .input(call_data.into()),
                )
                .await
                .map_err(|e| RpcMonitorError::Contract(e.to_string()))?;

            let l2_proven_block = if call_result.len() >= 32 {
                // Decode uint64 from ABI-encoded response (right-aligned in 32 bytes)
                let bytes: [u8; 8] = call_result[24..32]
                    .try_into()
                    .map_err(|_| {
                        RpcMonitorError::Contract(
                            "invalid response length".to_string(),
                        )
                    })?;
                u64::from_be_bytes(bytes)
            } else {
                warn!("oracle contract returned unexpected response, defaulting to 0");
                0
            };
            debug!(l2_proven_block, "fetched L2 proven block from oracle");

            Ok(ChainState {
                l1_head,
                l1_block_number,
                l2_proven_block,
                l2_safe_block,
                timestamp: SystemTime::now(),
            })
        }

        async fn active_dispute(&self) -> Option<DisputeInfo> {
            let dispute_address = self.dispute_address?;

            let l1_provider = ProviderBuilder::new()
                .connect_http(self.l1_rpc_url.clone());

            // Query isDisputed for a range of recent blocks
            // In production, this would scan DisputeCreated events
            // For now, we check the latest proven block
            let call_data =
                IOpenZkL2OutputOracle::latestBlockNumberCall {}.abi_encode();
            let call_result = l1_provider
                .call(
                    alloy_rpc_types_eth::TransactionRequest::default()
                        .to(self.oracle_address)
                        .input(call_data.into()),
                )
                .await
                .ok()?;

            if call_result.len() < 32 {
                return None;
            }

            let latest_block = u64::from_be_bytes(
                call_result[24..32].try_into().ok()?,
            );

            // Check if the latest block is disputed
            let is_disputed_data =
                open_zk_contracts::abi::IOpenZkDisputeGame::isDisputedCall {
                    blockNumber: latest_block,
                }
                .abi_encode();

            let disputed_result = l1_provider
                .call(
                    alloy_rpc_types_eth::TransactionRequest::default()
                        .to(dispute_address)
                        .input(is_disputed_data.into()),
                )
                .await
                .ok()?;

            if disputed_result.len() >= 32 && disputed_result[31] == 1 {
                Some(DisputeInfo {
                    start_block: latest_block,
                    end_block: latest_block,
                })
            } else {
                None
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn rpc_monitor_creation() {
            let monitor = RpcChainMonitor::new(
                "http://localhost:8545",
                "http://localhost:9545",
                Address::ZERO,
            )
            .unwrap();
            assert_eq!(monitor.l1_rpc_url.as_str(), "http://localhost:8545/");
        }

        #[test]
        fn rpc_monitor_with_dispute() {
            let monitor = RpcChainMonitor::new(
                "http://localhost:8545",
                "http://localhost:9545",
                Address::ZERO,
            )
            .unwrap()
            .with_dispute_address(Address::repeat_byte(0x42));

            assert_eq!(
                monitor.dispute_address,
                Some(Address::repeat_byte(0x42))
            );
        }

        #[test]
        fn rpc_monitor_invalid_url() {
            let result = RpcChainMonitor::new(
                "not a url",
                "http://localhost:9545",
                Address::ZERO,
            );
            assert!(result.is_err());
        }

        /// Integration test — requires running L1/L2 nodes + deployed contracts.
        #[tokio::test]
        #[ignore]
        async fn rpc_monitor_get_state_integration() {
            let monitor = RpcChainMonitor::new(
                "http://localhost:8545",
                "http://localhost:9545",
                Address::ZERO,
            )
            .unwrap();

            let state = monitor.get_state().await.unwrap();
            assert!(state.l1_block_number > 0);
        }
    }
}

#[cfg(not(feature = "rpc"))]
pub use skeleton::*;

#[cfg(feature = "rpc")]
pub use full::*;
