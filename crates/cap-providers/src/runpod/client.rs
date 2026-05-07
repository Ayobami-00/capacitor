use async_trait::async_trait;
use cap_core::{OfferObservation, WatchSpec};
use reqwest::{
    Url,
    header::{ACCEPT, CONTENT_TYPE, HeaderMap, HeaderValue},
};
use serde_json::json;

use crate::provider::{Provider, ProviderError};
use crate::runpod::normalize::normalize_gpu_types_response;

const RUNPOD_GRAPHQL_URL: &str = "https://api.runpod.io/graphql";

const GPU_TYPES_QUERY: &str = r#"
query CapacitorGpuTypes($gpuCount: Int!) {
  gpuTypes {
    id
    displayName
    memoryInGb
    lowestPrice(input: { gpuCount: $gpuCount, secureCloud: true }) {
      gpuName
      gpuTypeId
      uninterruptablePrice
      stockStatus
      countryCode
      availableGpuCounts
    }
  }
}
"#;

pub struct RunpodProvider {
    api_key: String,
    client: reqwest::Client,
}

impl RunpodProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Provider for RunpodProvider {
    fn name(&self) -> &'static str {
        "runpod"
    }

    async fn search(&self, spec: &WatchSpec) -> Result<Vec<OfferObservation>, ProviderError> {
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let gpu_count = spec.min_gpus.unwrap_or(1).max(1);
        let mut url = Url::parse(RUNPOD_GRAPHQL_URL)
            .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
        url.query_pairs_mut().append_pair("api_key", &self.api_key);

        let response = self
            .client
            .post(url)
            .headers(headers)
            .json(&json!({
                "query": GPU_TYPES_QUERY,
                "variables": {
                    "gpuCount": gpu_count,
                },
            }))
            .send()
            .await?
            .error_for_status()?;

        let body = response.text().await?;
        normalize_gpu_types_response(&body, spec)
    }
}
