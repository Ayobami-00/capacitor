use cap_core::{IngestBatch, IngestResult, InstallRegistration};
use reqwest::Url;
use serde::{Deserialize, Serialize};

pub const DEFAULT_INGEST_BASE_URL: &str = "https://jyfzizvtegmiukdjckfz.supabase.co";
pub const INGEST_BASE_URL: &str = match option_env!("CAPACITOR_INGEST_BASE_URL") {
    Some(url) => url,
    None => DEFAULT_INGEST_BASE_URL,
};

#[derive(Clone)]
pub struct IngestClient {
    client: reqwest::Client,
    base_url: Url,
}

#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("ingest token is missing; run `cap init` again")]
    MissingToken,
    #[error("invalid ingest API URL: {0}")]
    InvalidUrl(#[from] url::ParseError),
    #[error("ingest request failed: {0}")]
    Request(#[from] reqwest::Error),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RegistrationResponse {
    pub ingest_token: String,
}

impl IngestClient {
    pub fn fixed() -> Result<Self, IngestError> {
        Ok(Self {
            client: reqwest::Client::new(),
            base_url: Url::parse(INGEST_BASE_URL)?,
        })
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_base_url(base_url: Url) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
        }
    }

    pub async fn register(
        &self,
        registration: &InstallRegistration,
    ) -> Result<RegistrationResponse, IngestError> {
        let url = self.base_url.join("/functions/v1/ingest/init")?;
        let response = self
            .client
            .post(url)
            .json(registration)
            .send()
            .await?
            .error_for_status()?
            .json::<RegistrationResponse>()
            .await?;

        Ok(response)
    }

    pub async fn upload_observations(
        &self,
        token: Option<&str>,
        batch: &IngestBatch,
    ) -> Result<IngestResult, IngestError> {
        let token = token.ok_or(IngestError::MissingToken)?;
        let url = self.base_url.join("/functions/v1/ingest/obs")?;
        let response = self
            .client
            .post(url)
            .bearer_auth(token)
            .json(batch)
            .send()
            .await?
            .error_for_status()?
            .json::<IngestResult>()
            .await?;

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cap_core::IngestBatch;
    use uuid::Uuid;

    #[tokio::test]
    async fn upload_requires_token() {
        let client = IngestClient::fixed().unwrap();
        let batch = IngestBatch {
            cli_version: "0.1.0".to_string(),
            installation_id: Uuid::new_v4(),
            watch_run_id: Uuid::new_v4(),
            observations: Vec::new(),
        };

        assert!(matches!(
            client.upload_observations(None, &batch).await.unwrap_err(),
            IngestError::MissingToken
        ));
    }

    #[test]
    fn register_endpoint_is_public_init_route() {
        let client = IngestClient::fixed().unwrap();
        let url = client.base_url.join("/functions/v1/ingest/init").unwrap();

        assert_eq!(url.path(), "/functions/v1/ingest/init");
    }
}
