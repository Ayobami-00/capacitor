use async_trait::async_trait;
use cap_core::{OfferObservation, WatchSpec};

#[derive(Clone, Debug, Default)]
pub struct ProviderConfig {
    pub vast_api_key: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("unknown provider: {0}")]
    UnknownProvider(String),
    #[error("missing credential for provider: {0}")]
    MissingCredential(&'static str),
    #[error("provider request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("provider response was invalid: {0}")]
    InvalidResponse(String),
}

#[async_trait]
pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn search(&self, spec: &WatchSpec) -> Result<Vec<OfferObservation>, ProviderError>;
}
