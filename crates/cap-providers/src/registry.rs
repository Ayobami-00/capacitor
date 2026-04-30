use crate::provider::{Provider, ProviderConfig, ProviderError};
use crate::vast::VastProvider;

pub fn available_providers() -> &'static [&'static str] {
    &["vast"]
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
            vast_api_key: Some("test-token".to_string()),
        });

        assert_eq!(registry.build("vast").unwrap().name(), "vast");
    }

    #[test]
    fn rejects_unknown_provider() {
        let registry = ProviderRegistry::new(ProviderConfig::default());

        match registry.build("lambda") {
            Err(ProviderError::UnknownProvider(_)) => {}
            other => panic!("expected unknown provider error, got {}", other.is_ok()),
        }
    }
}
