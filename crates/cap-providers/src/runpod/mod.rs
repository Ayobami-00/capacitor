mod client;
mod models;
mod normalize;

pub use client::RunpodProvider;

#[cfg(test)]
mod tests {
    use cap_core::WatchSpec;

    use crate::runpod::normalize::normalize_gpu_types_response;

    #[test]
    fn normalizes_available_secure_cloud_gpu_types() {
        let raw = include_str!("../../../../fixtures/runpod/gpu_types_sample.json");
        let spec = WatchSpec {
            provider: "runpod".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(36.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: Some(8),
            poll_interval_secs: 60,
        };

        let observations = normalize_gpu_types_response(raw, &spec).unwrap();

        assert_eq!(observations.len(), 1);
        let observation = &observations[0];
        assert_eq!(observation.provider, "runpod");
        assert_eq!(
            observation.provider_offer_id,
            "NVIDIA H100 80GB HBM3:secure:8:US"
        );
        assert_eq!(observation.gpu_name, "NVIDIA H100 80GB HBM3");
        assert_eq!(observation.num_gpus, 8);
        assert_eq!(observation.gpu_ram_gb, Some(80.0));
        assert_eq!(observation.price_usd_per_hour, 34.32);
        assert_eq!(observation.reliability_score, Some(1.0));
        assert!(observation.verified);
        assert!(observation.rentable);
        assert_eq!(observation.region.as_deref(), Some("US"));
    }

    #[test]
    fn ignores_unavailable_or_unpriced_gpu_types() {
        let raw = include_str!("../../../../fixtures/runpod/gpu_types_no_capacity.json");
        let spec = WatchSpec {
            provider: "runpod".to_string(),
            gpu_filters: vec!["H100".to_string()],
            max_price: Some(36.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: Some(8),
            poll_interval_secs: 60,
        };

        let observations = normalize_gpu_types_response(raw, &spec).unwrap();

        assert!(observations.is_empty());
    }

    #[test]
    fn filters_runpod_observations_with_existing_watch_spec() {
        let raw = include_str!("../../../../fixtures/runpod/gpu_types_sample.json");
        let spec = WatchSpec {
            provider: "runpod".to_string(),
            gpu_filters: vec!["A100".to_string()],
            max_price: Some(10.0),
            verified: true,
            min_reliability: Some(0.98),
            min_gpus: Some(1),
            poll_interval_secs: 60,
        };

        let observations = normalize_gpu_types_response(raw, &spec).unwrap();

        assert_eq!(observations.len(), 1);
        assert_eq!(observations[0].gpu_name, "NVIDIA A100 SXM");
        assert_eq!(observations[0].num_gpus, 1);
    }
}
