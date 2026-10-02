use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};

fn copy_tree(source: &Path, dest: &Path, root: &Path, digest: &mut Sha256) {
    fs::create_dir_all(dest).expect("Create embedded web directory");
    let mut entries = fs::read_dir(source)
        .expect("Read web build")
        .map(|entry| entry.expect("Read web entry").path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let target = dest.join(path.file_name().expect("Web asset filename"));
        if path.is_dir() {
            copy_tree(&path, &target, root, digest);
        } else {
            println!("cargo:rerun-if-changed={}", path.display());
            let relative = path
                .strip_prefix(root)
                .expect("Asset within web root")
                .to_string_lossy();
            let contents = fs::read(&path).expect("Read embedded asset bytes");
            digest.update((relative.len() as u64).to_le_bytes());
            digest.update(relative.as_bytes());
            digest.update((contents.len() as u64).to_le_bytes());
            digest.update(&contents);
            fs::write(target, contents).expect("Copy embedded asset");
        }
    }
}
fn main() {
    println!("cargo:rerun-if-changed=../../web/dist");
    if std::env::var_os("CARGO_FEATURE_WEB").is_none() {
        return;
    }
    let source = Path::new("../../web/dist");
    let dest = PathBuf::from(std::env::var_os("OUT_DIR").expect("OUT_DIR")).join("web");
    if dest.exists() {
        fs::remove_dir_all(&dest).expect("Clear previous bundle");
    }
    let mut digest = Sha256::new();
    if source.join("index.html").is_file() {
        copy_tree(source, &dest, source, &mut digest);
    } else {
        if std::env::var("PROFILE").as_deref() == Ok("release") {
            panic!("Production UI missing: run npm --prefix web run build first");
        }
        fs::create_dir_all(&dest).expect("Create development hint");
        let hint = "<!doctype html><title>MoleAPI setup</title><p>Build the frontend with npm --prefix web run build, then rebuild the server.</p>";
        digest.update(hint.as_bytes());
        fs::write(dest.join("index.html"), hint).expect("Write development hint");
    }
    println!(
        "cargo:rustc-env=MOLEAPI_WEB_BUNDLE_DIGEST={:x}",
        digest.finalize()
    );
}
