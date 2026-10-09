use base64::{Engine, engine::general_purpose::STANDARD};
use moleapi_generation::project::*;
use serde_json::json;
use std::{collections::BTreeMap, path::PathBuf};
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "requires explicitly configured Java17+ test runtime"]
async fn all_seven_output_roles_and_static_assets_use_upstream_generation_and_roundtrip() {
    let java = PathBuf::from(
        std::env::var_os("MOLEAPI_CODEGEN_TEST_JAVA").expect("explicit Java executable"),
    );
    let runtime = ProjectRuntime::new(std::env::current_exe().unwrap(), Some(java)).unwrap();
    let definitions = [
        ("api", TemplateKind::Api),
        ("api_docs", TemplateKind::ApiDocs),
        ("api_tests", TemplateKind::ApiTests),
        ("model", TemplateKind::Model),
        ("model_docs", TemplateKind::ModelDocs),
        ("model_tests", TemplateKind::ModelTests),
        ("support", TemplateKind::SupportingFiles),
    ];
    let mut templates = TemplateBundle {
        format: "moleapi-codegen-templates-v1".into(),
        files: vec![],
        outputs: BTreeMap::new(),
    };
    for (name, kind) in definitions {
        let path = format!("custom_{name}.mustache");
        templates.files.push(TemplateFile {
            encoding: None,
            path: path.clone(),
            content: format!("OUTPUT_{name}: {{{{classname}}}} {{{{appName}}}}\n"),
        });
        templates.outputs.insert(
            path,
            TemplateOutput {
                folder: if kind == TemplateKind::SupportingFiles {
                    "extras".into()
                } else {
                    String::new()
                },
                destination_filename: Some(format!("{name}.txt")),
                template_type: kind,
            },
        );
    }
    templates.files.push(TemplateFile {
        encoding: None,
        path: "AUTHORS.md".into(),
        content: "Static authors {{unprocessed}}\n".into(),
    });
    templates
        .outputs
        .insert("AUTHORS.md".into(), TemplateOutput::default());
    templates.files.push(TemplateFile {
        encoding: None,
        path: "static/CREDITS.md".into(),
        content: "Nested static credits\n".into(),
    });
    templates
        .outputs
        .insert("static/CREDITS.md".into(), TemplateOutput::default());
    let png=STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=").unwrap();
    templates.files.push(TemplateFile {
        path: "assets/pixel.png".into(),
        content: STANDARD.encode(&png),
        encoding: Some("base64".into()),
    });
    templates.outputs.insert(
        "assets/pixel.png".into(),
        TemplateOutput {
            template_type: TemplateKind::SupportingFiles,
            folder: "images".into(),
            destination_filename: Some("pixel.png".into()),
        },
    );
    let input = ProjectInput {
        target: "typescript-fetch".into(),
        include_secrets: false,
        options: BTreeMap::new(),
        templates: Some(templates.clone()),
        specification: json!({"openapi":"3.0.3","info":{"title":"Output fixture","version":"1"},"paths":{"/pet":{"get":{"operationId":"getPet","tags":["Pets"],"responses":{"200":{"description":"Pet","content":{"application/json":{"schema":{"$ref":"#/components/schemas/Pet"}}}}}}}},"components":{"schemas":{"Pet":{"type":"object","properties":{"id":{"type":"integer"}}}}}}),
    };
    let result = runtime
        .generate(input, CancellationToken::new())
        .await
        .unwrap();
    for name in [
        "api",
        "api_docs",
        "api_tests",
        "model",
        "model_docs",
        "model_tests",
        "support",
    ] {
        assert!(
            result
                .files
                .iter()
                .any(|file| file.path != "moleapi-templates.json"
                    && file.content.contains(&format!("OUTPUT_{name}:"))),
            "missing {name}: {:?}",
            result
                .files
                .iter()
                .map(|file| &file.path)
                .collect::<Vec<_>>()
        );
    }
    assert!(
        result.files.iter().any(
            |file| file.path == "extras/support.txt" && file.content.contains("Output fixture")
        )
    );
    assert!(result.files.iter().any(
        |file| file.path == "AUTHORS.md" && file.content == "Static authors {{unprocessed}}\n"
    ));
    assert!(
        result
            .files
            .iter()
            .any(|file| file.path == "static/CREDITS.md"
                && file.content == "Nested static credits\n")
    );
    let image = result
        .files
        .iter()
        .find(|file| file.path == "images/pixel.png")
        .unwrap();
    assert_eq!(image.encoding, "base64");
    assert_eq!(STANDARD.decode(&image.content).unwrap(), png);
    let saved = result
        .files
        .iter()
        .find(|file| file.path == "moleapi-templates.json")
        .unwrap();
    let restored: TemplateBundle = serde_json::from_str(&saved.content).unwrap();
    restored.validate().unwrap();
    assert_eq!(
        serde_json::to_value(restored).unwrap(),
        serde_json::to_value(templates).unwrap()
    );
    assert!(
        !result
            .files
            .iter()
            .find(|file| file.path == "AUTHORS.md")
            .unwrap()
            .executable
    );
}
