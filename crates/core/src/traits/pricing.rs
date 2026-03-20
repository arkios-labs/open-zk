use crate::types::{CycleEstimate, PricingInfo, ZkvmBackend};
use async_trait::async_trait;

/// Converts cycle counts into USD pricing and duration estimates.
#[async_trait]
pub trait PricingProvider: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    fn name(&self) -> &str;
    fn supports_backend(&self, backend: ZkvmBackend) -> bool;
    async fn price(&self, estimate: &CycleEstimate) -> Result<PricingInfo, Self::Error>;
}
