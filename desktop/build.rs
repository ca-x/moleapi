fn main() {
    if std::env::var("PROFILE").as_deref() == Ok("release") {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../vendor/java-runtime/current");
        let receipt = root.join("moleapi-java-runtime.json");
        println!("cargo:rerun-if-changed={}", receipt.display());
        let value: serde_json::Value = serde_json::from_slice(
            &std::fs::read(receipt)
                .expect("Prepare the pinned Java runtime before packaging desktop releases"),
        )
        .expect("Invalid bundled Java receipt");
        assert_eq!(
            value["target"].as_str(),
            std::env::var("TARGET").ok().as_deref(),
            "Prepared Java runtime does not match desktop target"
        );
        let config: serde_json::Value = serde_json::from_str(
            &std::env::var("TAURI_CONFIG").unwrap_or_default(),
        )
        .expect("Use the generated tauri.java.json resource override when building releases");
        assert!(
            config["bundle"]["resources"]
                .as_object()
                .is_some_and(|resources| resources
                    .values()
                    .any(|value| value.as_str() == Some("runtime/java/"))),
            "Desktop releases must include the prepared runtime/java resource directory"
        );
    }
    tauri_build::build()
}
