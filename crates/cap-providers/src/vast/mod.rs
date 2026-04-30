mod client;
mod models;
mod normalize;

pub use client::VastProvider;

#[cfg(test)]
mod tests {
    use super::normalize::normalize_search_response;
    use cap_core::WatchSpec;

    #[test]
    fn normalizes_vast_fixture() {
        let raw = include_str!("../../../../fixtures/vast/search_offers_sample.json");
        let spec = WatchSpec {
            provider: "vast".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(3.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: None,
            poll_interval_secs: 60,
        };

        let observations = normalize_search_response(raw, &spec).unwrap();
        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].gpu_name, "H100 SXM");
        assert!(observations[0].verified);
    }
}
