use super::*;
use anyhow::{Context, ensure};
use std::{
    ffi::OsStr,
    io::{Read, Write},
};
pub const WORKER_ARG: &str = "--moleapi-project-worker";
/// Runs before application initialization; the caller installs a native heap cap.
pub fn dispatch_project_worker(limit: impl FnOnce() -> Result<()>) -> Result<bool> {
    if std::env::args_os().nth(1).as_deref() != Some(OsStr::new(WORKER_ARG)) {
        return super::regeneration::dispatch_diff_worker(limit);
    }
    limit()?;
    let mut bytes = Vec::new();
    std::io::stdin()
        .lock()
        .take((SPEC_LIMIT + 65537) as u64)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= SPEC_LIMIT + 65536,
        "Project worker input limit exceeded"
    );
    let input: ProjectInput = serde_json::from_slice(&bytes)
        .map_err(|_| anyhow::anyhow!("Invalid project worker input"))?;
    let result = std::panic::catch_unwind(|| native_project(&input));
    let reply = match result {
        Ok(Ok(artifact)) => serde_json::json!({"artifact":artifact}),
        _ => {
            serde_json::json!({"error":"Rust SDK generation failed; check the specification and target options"})
        }
    };
    let output = serde_json::to_vec(&reply)?;
    ensure!(
        output.len() <= IPC_LIMIT,
        "Project worker reply limit exceeded"
    );
    std::io::stdout().lock().write_all(&output)?;
    Ok(true)
}
fn native_project(input: &ProjectInput) -> Result<ProjectArtifact> {
    if input.target == "rust-tonic" {
        return super::protobuf::native_protobuf(input);
    }
    validate_project_specification(&input.specification)?;
    validate_options(input)?;
    ensure!(
        matches!(
            input.target.as_str(),
            "rust-progenitor" | "rust-progenitor-cli"
        ),
        "Unsupported native generator"
    );
    ensure!(
        input
            .options
            .keys()
            .all(|k| ["packageName", "packageVersion", "interface"].contains(&k.as_str())),
        "Unsupported native Rust SDK option"
    );
    let package = input
        .options
        .get("packageName")
        .and_then(Value::as_str)
        .unwrap_or("moleapi_sdk");
    ensure!(
        package.len() <= 64
            && package.starts_with(|c: char| c.is_ascii_alphabetic())
            && package
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-'),
        "Invalid Rust package name"
    );
    let version = input
        .options
        .get("packageVersion")
        .and_then(Value::as_str)
        .unwrap_or("0.1.0");
    semver::Version::parse(version).context("Invalid package version")?;
    ensure!(
        input.specification["openapi"]
            .as_str()
            .is_some_and(|v| v.starts_with("3.0.")),
        "Progenitor supports OpenAPI3.0; select an OpenAPI Generator target for3.1"
    );
    let spec: openapiv3::OpenAPI = serde_json::from_value(input.specification.clone())?;
    let is_cli = input.target == "rust-progenitor-cli";
    let mut settings = progenitor::GenerationSettings::default();
    if is_cli {
        settings
            .with_interface(progenitor::InterfaceStyle::Builder)
            .with_derive("schemars::JsonSchema");
    }
    if let Some(style) = input.options.get("interface") {
        settings.with_interface(match style.as_str() {
            Some("builder") => progenitor::InterfaceStyle::Builder,
            Some("positional") => progenitor::InterfaceStyle::Positional,
            _ => anyhow::bail!("Unknown Rust SDK interface style"),
        });
    }
    let mut generator = progenitor::Generator::new(&settings);
    let tokens = generator.generate_tokens(&spec)?;
    let syntax: syn::File = syn::parse2(tokens)?;
    let mut code = prettyplease::unparse(&syntax);
    let mut manifest = format!(
        "[package]\nname = {package:?}\nversion = {version:?}\nedition = \"2021\"\n\n[dependencies]\nprogenitor-client = \"=0.15.0\"\nreqwest = {{ version=\"0.13\",default-features=false,features=[\"rustls\",\"json\",\"query\",\"stream\"] }}\nserde = {{version=\"1\",features=[\"derive\"]}}\nserde_json = \"1\"\nfutures = \"0.3\"\nchrono = {{version=\"0.4\",features=[\"serde\"]}}\nuuid = {{version=\"1\",features=[\"serde\",\"v4\"]}}\nbase64 = \"0.22\"\nrand = \"0.9\"\n"
    );
    let mut files=BTreeMap::from([("Cargo.toml".into(),manifest.clone().into_bytes()),("src/lib.rs".into(),code.clone().into_bytes()),("openapi.json".into(),serde_json::to_vec_pretty(&input.specification)?),("README.md".into(),b"Generated with Progenitor 0.15.0. Configure Client with an explicit service base URL. Generated output is separate from your authored files; regenerate into a new directory and inspect diffs.\n".to_vec())]);
    if is_cli {
        let mut cli_generator = progenitor::Generator::new(&settings);
        let cli_tokens = cli_generator.cli(&spec, "crate")?;
        let cli_file: syn::File = syn::parse2(cli_tokens)?;
        let cli_source = prettyplease::unparse(&cli_file);
        ensure!(
            !cli_source.contains("todo!()"),
            "Native CLI generator does not yet support upgrade or raw paginated responses; use a different CLI target"
        );
        code.push_str("\npub mod cli;\n");
        files.insert("src/lib.rs".into(), code.into_bytes());
        files.insert("src/cli.rs".into(), cli_source.into_bytes());
        files.insert(
            "src/main.rs".into(),
            include_str!("cli_main.rs.txt")
                .replace("__SDK_CRATE__", &package.replace('-', "_"))
                .into_bytes(),
        );
        manifest.push_str("anyhow = \"1\"\nclap = {version=\"4\",features=[\"env\"]}\nschemars = {version=\"0.8\",features=[\"chrono\",\"uuid1\"]}\ntokio = {version=\"1\",features=[\"macros\",\"rt-multi-thread\",\"fs\",\"io-util\",\"io-std\"]}\n");
        files.insert("Cargo.toml".into(), manifest.into_bytes());
        files.insert("README.md".into(),b"Generated CLI uses Progenitor's clap operation parser and typed SDK. Build with cargo build. Set MOLEAPI_API_URL or --base-url and choose an operation with --help. Optional MOLEAPI_API_TOKEN supplies a Bearer token; no credentials are baked into generated code. JSON results go to stdout; binary responses stream unchanged to stdout or --output-file (new files only). Use operation --body-file for raw binary/text request bodies (loaded into memory). Errors exit nonzero.\n".to_vec());
    }
    artifact(
        input,
        "progenitor@0.15.0",
        files
            .into_iter()
            .map(|(path, bytes)| (path, bytes.into()))
            .collect(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_models_and_methods_are_mature_generator_output() {
        let input = ProjectInput {
            target: "rust-progenitor".into(),
            options: BTreeMap::new(),
            include_secrets: false,
            specification: serde_json::json!({"openapi":"3.0.3","info":{"title":"Fixture","version":"1"},"paths":{"/health":{"get":{"operationId":"health","responses":{"200":{"description":"OK","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Health"}}}}}}}},"components":{"schemas":{"Health":{"type":"object","required":["ok"],"properties":{"ok":{"type":"boolean"}}}}}}),
        };
        let result = native_project(&input).unwrap();
        assert!(
            result
                .files
                .iter()
                .any(|f| f.path == "src/lib.rs" && f.content.contains("pub async fn health"))
        );
        assert!(result.files.iter().any(|f| f.path == "Cargo.toml"));
    }
    #[test]
    #[ignore = "Compiles a generated standalone project; run explicitly with a cached Cargo toolchain"]
    fn native_cli_streams_binary_and_returns_raw_errors() {
        use std::{net::TcpListener, process::Command};
        let input = ProjectInput {
            target: "rust-progenitor-cli".into(),
            options: BTreeMap::new(),
            include_secrets: false,
            specification: serde_json::json!({
                "openapi":"3.0.3", "info":{"title":"Binary fixture","version":"1"},
                "paths":{
                    "/download":{"get":{"operationId":"download","responses":{
                        "200":{"description":"Bytes","content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}}
                    }}},
                    "/upload":{"post":{"operationId":"upload","requestBody":{"required":true,"content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}},"responses":{
                        "204":{"description":"Uploaded"}
                    }}},
                    "/failure":{"get":{"operationId":"failure","responses":{
                        "204":{"description":"Empty"},
                        "400":{"description":"Binary error","content":{"application/octet-stream":{"schema":{"type":"string","format":"binary"}}}}
                    }}}
                }
            }),
        };
        let artifact = native_project(&input).unwrap();
        let project = tempfile::tempdir().unwrap();
        for file in &artifact.files {
            let path = project.path().join(&file.path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, file.content.as_bytes()).unwrap();
        }
        let target = std::env::var_os("MOLEAPI_GENERATED_TEST_TARGET")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| project.path().join("target"));
        let build = Command::new("cargo")
            .args(["build", "--quiet"])
            .current_dir(project.path())
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let payload = vec![0, 255, 128, b'\n', b'X'];
        let response = payload.clone();
        let server = std::thread::spawn(move || {
            for _ in 0..5 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(std::time::Duration::from_secs(10)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    request.push(byte[0]);
                }
                if request.starts_with(b"POST /upload ") {
                    let headers = String::from_utf8_lossy(&request);
                    let length = headers
                        .lines()
                        .find_map(|line| {
                            let (name, value) = line.split_once(':')?;
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                        .unwrap();
                    assert_eq!(length, response.len());
                    let mut body = vec![0; length];
                    socket.read_exact(&mut body).unwrap();
                    assert_eq!(body, response);
                    socket
                        .write_all(b"HTTP/1.1 204 No Content\r\nConnection: close\r\n\r\n")
                        .unwrap();
                    continue;
                }
                let status = if request.starts_with(b"GET /failure ") {
                    "400 Bad Request"
                } else {
                    "200 OK"
                };
                write!(socket, "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", response.len()).unwrap();
                socket.write_all(&response).unwrap();
            }
        });
        let executable = target
            .join("debug")
            .join(format!("moleapi_sdk{}", std::env::consts::EXE_SUFFIX));
        let run = |args: &[&str]| {
            Command::new(&executable)
                .args(["--base-url", &url])
                .args(args)
                .output()
                .unwrap()
        };
        let stdout = run(&["download"]);
        assert!(
            stdout.status.success(),
            "{}",
            String::from_utf8_lossy(&stdout.stderr)
        );
        assert_eq!(stdout.stdout, payload);
        let file = project.path().join("download.bin");
        let saved = run(&["--output-file", file.to_str().unwrap(), "download"]);
        assert!(
            saved.status.success(),
            "{}",
            String::from_utf8_lossy(&saved.stderr)
        );
        assert!(saved.stdout.is_empty());
        assert_eq!(std::fs::read(&file).unwrap(), payload);
        let overwrite = run(&["--output-file", file.to_str().unwrap(), "download"]);
        assert!(!overwrite.status.success());
        assert_eq!(std::fs::read(&file).unwrap(), payload);
        let upload = run(&["upload", "--body-file", file.to_str().unwrap()]);
        assert!(
            upload.status.success(),
            "{}",
            String::from_utf8_lossy(&upload.stderr)
        );
        assert!(upload.stdout.is_empty());
        let error = run(&["failure"]);
        assert!(!error.status.success());
        assert!(error.stdout.is_empty());
        assert!(String::from_utf8_lossy(&error.stderr).contains("400"));
        server.join().unwrap();
    }
}
