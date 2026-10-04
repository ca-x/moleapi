use sha2::{Digest, Sha256};
fn main() {
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
}
