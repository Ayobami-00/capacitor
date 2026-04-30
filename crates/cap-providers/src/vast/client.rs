use async_trait::async_trait;
use cap_core::{OfferObservation, WatchSpec};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde_json::{Value, json};

use crate::provider::{Provider, ProviderError};
use crate::vast::normalize::normalize_search_response;

const VAST_SEARCH_OFFERS_URL: &str = "https://console.vast.ai/api/v0/bundles/";

pub struct VastProvider {
    api_key: String,
    client: reqwest::Client,
}

impl VastProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }

    fn search_body(spec: &WatchSpec) -> Value {
        let mut body = json!({
            "limit": 100,
            "type": "ondemand",
            "rentable": { "eq": true },
            "rented": { "eq": false }
        });

        if spec.verified {
            body["verified"] = json!({ "eq": true });
        }

        if let Some(max_price) = spec.max_price {
            body["dph_total"] = json!({ "lte": max_price });
        }

        if let Some(min_reliability) = spec.min_reliability {
            body["reliability"] = json!({ "gte": min_reliability });
        }

        if let Some(min_gpus) = spec.min_gpus {
            body["num_gpus"] = json!({ "gte": min_gpus });
        }

        body
    }
}

#[async_trait]
impl Provider for VastProvider {
    fn name(&self) -> &'static str {
        "vast"
    }

    async fn search(&self, spec: &WatchSpec) -> Result<Vec<OfferObservation>, ProviderError> {
        let mut headers = HeaderMap::new();
        let authorization = format!("Bearer {}", self.api_key);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&authorization)
                .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?,
        );
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

        let response = self
            .client
            .post(VAST_SEARCH_OFFERS_URL)
            .headers(headers)
            .json(&Self::search_body(spec))
            .send()
            .await?
            .error_for_status()?;

        let body = response.text().await?;
        normalize_search_response(&body, spec)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_search_body_from_watch_spec() {
        let spec = WatchSpec {
            provider: "vast".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(3.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: Some(8),
            poll_interval_secs: 60,
        };

        let body = VastProvider::search_body(&spec);
        assert_eq!(body["rentable"]["eq"], true);
        assert_eq!(body["rented"]["eq"], false);
        assert_eq!(body["verified"]["eq"], true);
        assert_eq!(body["dph_total"]["lte"], 3.0);
        assert_eq!(body["reliability"]["gte"], 0.98);
        assert_eq!(body["num_gpus"]["gte"], 8);
    }
}
