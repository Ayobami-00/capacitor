use async_trait::async_trait;
use cap_core::{OfferObservation, WatchSpec};
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderMap, HeaderValue};

use crate::lambda::normalize::normalize_instance_types_response;
use crate::provider::{Provider, ProviderError};

const LAMBDA_INSTANCE_TYPES_URL: &str = "https://cloud.lambda.ai/api/v1/instance-types";

pub struct LambdaProvider {
    api_key: String,
    client: reqwest::Client,
}

impl LambdaProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Provider for LambdaProvider {
    fn name(&self) -> &'static str {
        "lambda"
    }

    async fn search(&self, spec: &WatchSpec) -> Result<Vec<OfferObservation>, ProviderError> {
        let mut headers = HeaderMap::new();
        let authorization = format!("Bearer {}", self.api_key);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&authorization)
                .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?,
        );
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

        let response = self
            .client
            .get(LAMBDA_INSTANCE_TYPES_URL)
            .headers(headers)
            .send()
            .await?
            .error_for_status()?;

        let body = response.text().await?;
        normalize_instance_types_response(&body, spec)
    }
}
