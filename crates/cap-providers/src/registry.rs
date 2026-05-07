use crate::lambda::LambdaProvider;
use crate::provider::{Provider, ProviderConfig, ProviderError};
use crate::runpod::RunpodProvider;
use crate::vast::VastProvider;

pub fn available_providers() -> &'static [&'static str] {
    &["vast", "lambda", "runpod"]
}

pub struct ProviderRegistry {
    config: ProviderConfig,
}

impl ProviderRegistry {
    pub fn new(config: ProviderConfig) -> Self {
        Self { config }
    }

    pub fn build(&self, name: &str) -> Result<Box<dyn Provider>, ProviderError> {
        match name.to_ascii_lowercase().as_str() {
            "lambda" => {
                let api_key = self
                    .config
                    .lambda_api_key
                    .clone()
                    .ok_or(ProviderError::MissingCredential("lambda"))?;
                Ok(Box::new(LambdaProvider::new(api_key)))
            }
            "runpod" => {
                let api_key = self
                    .config
                    .runpod_api_key
                    .clone()
                    .ok_or(ProviderError::MissingCredential("runpod"))?;
                Ok(Box::new(RunpodProvider::new(api_key)))
            }
            "vast" => {
                let api_key = self
                    .config
                    .vast_api_key
                    .clone()
                    .ok_or(ProviderError::MissingCredential("vast"))?;
                Ok(Box::new(VastProvider::new(api_key)))
            }
            other => Err(ProviderError::UnknownProvider(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_vast_provider() {
        let registry = ProviderRegistry::new(ProviderConfig {
            lambda_api_key: None,
            runpod_api_key: None,
            vast_api_key: Some("test-token".to_string()),
        });

        assert_eq!(registry.build("vast").unwrap().name(), "vast");
    }

    #[test]
    fn resolves_lambda_provider() {
        let registry = ProviderRegistry::new(ProviderConfig {
            lambda_api_key: Some("test-token".to_string()),
            runpod_api_key: None,
            vast_api_key: None,
        });

        assert_eq!(registry.build("lambda").unwrap().name(), "lambda");
    }

    #[test]
    fn resolves_runpod_provider() {
        let registry = ProviderRegistry::new(ProviderConfig {
            lambda_api_key: None,
            runpod_api_key: Some("test-token".to_string()),
            vast_api_key: None,
        });

        assert_eq!(registry.build("runpod").unwrap().name(), "runpod");
    }

    #[test]
    fn rejects_lambda_without_api_key() {
        let registry = ProviderRegistry::new(ProviderConfig::default());

        match registry.build("lambda") {
            Err(ProviderError::MissingCredential("lambda")) => {}
            other => panic!(
                "expected missing lambda credential error, got success={}",
                other.is_ok()
            ),
        }
    }

    #[test]
    fn rejects_runpod_without_api_key() {
        let registry = ProviderRegistry::new(ProviderConfig::default());

        match registry.build("runpod") {
            Err(ProviderError::MissingCredential("runpod")) => {}
            other => panic!(
                "expected missing runpod credential error, got success={}",
                other.is_ok()
            ),
        }
    }

    #[test]
    fn rejects_unknown_provider() {
        let registry = ProviderRegistry::new(ProviderConfig::default());

        match registry.build("unknown") {
            Err(ProviderError::UnknownProvider(_)) => {}
            other => panic!("expected unknown provider error, got {}", other.is_ok()),
        }
    }
}
