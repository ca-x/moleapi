use sha2::{Digest, Sha256};
fn main() {
    let model_directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/model-engine");
    let model_manifest = model_directory.join("manifest.json");
    let model_bundle = model_directory.join("engine.js");
    println!("cargo:rerun-if-changed={}", model_manifest.display());
    println!("cargo:rerun-if-changed={}", model_bundle.display());
    let model_metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(model_manifest).expect("Model manifest missing"))
            .expect("Model manifest JSON");
    assert_eq!(model_metadata["version"], "26.0.0");
    assert_eq!(
        model_metadata["schema_converter"],
        "@openapi-contrib/openapi-schema-to-json-schema@5.1.0"
    );
    assert_eq!(
        format!(
            "{:x}",
            Sha256::digest(std::fs::read(model_bundle).expect("Model bundle missing"))
        ),
        model_metadata["sha256"]
            .as_str()
            .expect("Model bundle digest"),
        "Model bundle digest mismatch"
    );
    let engine =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/openapi-generator");
    let metadata_path = engine.join("manifest.json");
    let jar = engine.join("engine.jar");
    println!("cargo:rerun-if-changed={}", metadata_path.display());
    println!("cargo:rerun-if-changed={}", jar.display());
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(metadata_path).expect("Generator manifest missing"))
            .expect("Generator manifest JSON");
    assert_eq!(
        metadata["version"], "7.26.0",
        "OpenAPI engine version must match runtime metadata"
    );
    assert_eq!(
        metadata["sha256"], "5ccb60c4678901715376c6114afba60c28d0844fe506bd1e02891cbc363ab9ff",
        "OpenAPI engine digest must match runtime metadata"
    );
    let bytes = std::fs::read(&jar).unwrap_or_default();
    if bytes.is_empty() {
        assert!(
            std::env::var("PROFILE").as_deref() != Ok("release"),
            "Run python scripts/prepare_openapi_generator.py before release build"
        );
    } else {
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            metadata["sha256"].as_str().unwrap(),
            "OpenAPI generator asset digest mismatch"
        );
    }
    std::fs::write(
        std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("openapi-generator.jar"),
        bytes,
    )
    .expect("Copy embedded generator");
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor/snippet-engine");
    let bundle = directory.join("engine.js");
    let manifest = directory.join("manifest.json");
    println!("cargo:rerun-if-changed={}", bundle.display());
    println!("cargo:rerun-if-changed={}", manifest.display());
    let bytes = std::fs::read(bundle).expect("Embedded snippet bundle must exist");
    let metadata: serde_json::Value =
        serde_json::from_slice(&std::fs::read(manifest).expect("Snippet manifest must exist"))
            .expect("Snippet manifest must be JSON");
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        metadata["sha256"].as_str().expect("Snippet digest"),
        "Snippet bundle digest mismatch; regenerate from pinned sources"
    );
    assert_eq!(
        metadata["version"], "0.10.5",
        "Engine version must match Rust metadata"
    );
    assert_eq!(
        metadata["supplemental_engine"], "postman-code-generators@2.1.1",
        "Supplemental emitter must match the pinned runtime"
    );
    assert_eq!(
        metadata["collection_sdk"], "postman-collection@5.3.1",
        "Collection adapter must match the pinned runtime"
    );
}
