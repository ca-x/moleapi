//! Portable protobuf sources compiled by protox; JSON semantics supplied by prost-reflect.
use crate::{Protocol, RequestSpec, Specification};
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use prost::Message;
use prost_reflect::{
    DescriptorPool, DynamicMessage, Kind, MessageDescriptor, MethodDescriptor, SerializeOptions,
};
use protox::file::{File, FileResolver, GoogleFileResolver};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MAX_PROTO_BYTES: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ProtobufSource {
    Proto {
        files: Vec<ProtoFile>,
        entry_files: Vec<String>,
    },
    Descriptor {
        descriptor_set_base64: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtoFile {
    pub path: String,
    pub content: String,
}
#[derive(Clone, Debug, Serialize)]
pub struct GrpcSchema {
    pub services: Vec<GrpcService>,
    pub messages: Vec<GrpcMessageType>,
    pub enums: Vec<GrpcEnumType>,
}
#[derive(Clone, Debug, Serialize)]
pub struct GrpcMessageType {
    pub name: String,
    pub fields: Vec<GrpcField>,
}
#[derive(Clone, Debug, Serialize)]
pub struct GrpcField {
    pub name: String,
    pub json_name: String,
    pub number: u32,
    pub type_name: String,
    pub repeated: bool,
    pub map: bool,
    pub optional: bool,
    pub oneof: Option<String>,
}
#[derive(Clone, Debug, Serialize)]
pub struct GrpcEnumType {
    pub name: String,
    pub values: Vec<GrpcEnumValue>,
}
#[derive(Clone, Debug, Serialize)]
pub struct GrpcEnumValue {
    pub name: String,
    pub number: i32,
}
#[derive(Clone, Debug, Serialize)]
pub struct GrpcService {
    pub name: String,
    pub methods: Vec<GrpcMethod>,
}
#[derive(Clone, Debug, Serialize)]
pub struct GrpcMethod {
    pub name: String,
    pub full_name: String,
    pub input_type: String,
    pub output_type: String,
    pub client_streaming: bool,
    pub server_streaming: bool,
    pub input_template: serde_json::Value,
}
pub fn validate_proto_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && path.len() <= 256
            && path.ends_with(".proto")
            && !path.contains(['\\', ':'])
            && !path.starts_with('/')
            && path.split('/').all(|s| !s.is_empty()
                && s != "."
                && s != ".."
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))),
        "Unsafe protobuf virtual path"
    );
    Ok(())
}
struct VirtualFiles(HashMap<String, String>);
impl FileResolver for VirtualFiles {
    fn open_file(&self, name: &str) -> Result<File, protox::Error> {
        if validate_proto_path(name).is_err() {
            return Err(protox::Error::file_not_found(name));
        }
        if let Some(source) = self.0.get(name) {
            File::from_source(name, source)
        } else {
            GoogleFileResolver::new().open_file(name)
        }
    }
}
pub fn protobuf_pool(spec: &Specification) -> Result<DescriptorPool> {
    ensure!(
        spec.kind == "protobuf",
        "Request requires a protobuf specification"
    );
    ensure!(
        spec.source.len() <= MAX_PROTO_BYTES * 2,
        "Protobuf source exceeds 4 MiB"
    );
    let source: ProtobufSource =
        serde_json::from_str(&spec.source).context("Invalid protobuf source bundle")?;
    let bytes = match source {
        ProtobufSource::Descriptor {
            descriptor_set_base64,
        } => {
            ensure!(
                descriptor_set_base64.len() <= MAX_PROTO_BYTES.div_ceil(3) * 4,
                "Descriptor source exceeds 2 MiB"
            );
            STANDARD
                .decode(descriptor_set_base64)
                .context("Invalid descriptor base64")?
        }
        ProtobufSource::Proto { files, entry_files } => {
            ensure!(
                !files.is_empty()
                    && files.len() <= 32
                    && !entry_files.is_empty()
                    && entry_files.len() <= 32,
                "Proto bundle requires 1–32 files and entries"
            );
            let mut sources = HashMap::new();
            let mut total = 0usize;
            for file in files {
                validate_proto_path(&file.path)?;
                total += file.content.len();
                ensure!(
                    total <= MAX_PROTO_BYTES && file.content.len() <= 256 * 1024,
                    "Proto source byte limit exceeded"
                );
                ensure!(
                    sources.insert(file.path, file.content).is_none(),
                    "Duplicate proto file path"
                );
            }
            for entry in &entry_files {
                validate_proto_path(entry)?;
                ensure!(sources.contains_key(entry), "Proto entry file is missing");
            }
            // Only virtual files and library well-known descriptors: never filesystem imports.
            let mut compiler = protox::Compiler::with_file_resolver(VirtualFiles(sources));
            compiler.include_imports(true).include_source_info(false);
            compiler.open_files(entry_files)?;
            compiler.encode_file_descriptor_set()
        }
    };
    ensure!(bytes.len() <= MAX_PROTO_BYTES, "Descriptor exceeds 2 MiB");
    // Prost decoder enforces its default recursion limit before descriptor validation.
    let pool = DescriptorPool::decode(bytes.as_slice()).context("Invalid protobuf descriptors")?;
    ensure!(
        pool.files().count() <= 64 && pool.all_messages().count() <= 2048,
        "Descriptor file/type limit exceeded"
    );
    ensure!(
        pool.services().count() <= 256
            && pool
                .services()
                .map(|service| service.methods().count())
                .sum::<usize>()
                <= 1024,
        "Descriptor service/method limit exceeded"
    );
    ensure!(
        pool.all_messages()
            .all(|message| message.fields().count() <= 1024)
            && pool
                .all_messages()
                .map(|message| message.fields().count())
                .sum::<usize>()
                <= 16384,
        "Descriptor field limit exceeded"
    );
    Ok(pool)
}
pub fn grpc_schema(pool: &DescriptorPool) -> Result<GrpcSchema> {
    Ok(GrpcSchema {
        services: pool
            .services()
            .map(|service| GrpcService {
                name: service.full_name().into(),
                methods: service
                    .methods()
                    .map(|method| GrpcMethod {
                        name: method.name().into(),
                        full_name: method.full_name().into(),
                        input_type: method.input().full_name().into(),
                        output_type: method.output().full_name().into(),
                        client_streaming: method.is_client_streaming(),
                        server_streaming: method.is_server_streaming(),
                        input_template: message_template(method.input(), 0, &mut 1024),
                    })
                    .collect(),
            })
            .collect(),
        messages: pool
            .all_messages()
            .map(|message| GrpcMessageType {
                name: message.full_name().into(),
                fields: message
                    .fields()
                    .map(|field| GrpcField {
                        name: field.name().into(),
                        json_name: field.json_name().into(),
                        number: field.number(),
                        type_name: match field.kind() {
                            Kind::Message(m) => m.full_name().into(),
                            Kind::Enum(e) => e.full_name().into(),
                            scalar => format!("{scalar:?}").to_lowercase(),
                        },
                        repeated: field.is_list(),
                        map: field.is_map(),
                        optional: field.supports_presence(),
                        oneof: field.containing_oneof().map(|oneof| oneof.name().into()),
                    })
                    .collect(),
            })
            .collect(),
        enums: pool
            .all_enums()
            .map(|enumeration| GrpcEnumType {
                name: enumeration.full_name().into(),
                values: enumeration
                    .values()
                    .map(|value| GrpcEnumValue {
                        name: value.name().into(),
                        number: value.number(),
                    })
                    .collect(),
            })
            .collect(),
    })
}
fn message_template(
    desc: MessageDescriptor,
    depth: usize,
    budget: &mut usize,
) -> serde_json::Value {
    if depth >= 4 {
        return serde_json::json!({});
    }
    if desc.full_name().starts_with("google.protobuf.") {
        return grpc_json(&DynamicMessage::new(desc)).unwrap_or(serde_json::json!({}));
    }
    let mut object = serde_json::Map::new();
    for field in desc.fields() {
        if *budget == 0 {
            break;
        }
        *budget -= 1;
        let value = if field.is_list() {
            serde_json::json!([])
        } else if field.is_map() {
            serde_json::json!({})
        } else {
            match field.kind() {
                Kind::Message(m) => match m.full_name() {
                    "google.protobuf.Timestamp" => serde_json::json!("1970-01-01T00:00:00Z"),
                    "google.protobuf.Duration" => serde_json::json!("0s"),
                    "google.protobuf.FieldMask" => serde_json::json!(""),
                    "google.protobuf.Value" => serde_json::Value::Null,
                    _ => message_template(m, depth + 1, budget),
                },
                Kind::Enum(e) => serde_json::json!(e.default_value().name()),
                Kind::String | Kind::Bytes => serde_json::json!(""),
                Kind::Bool => serde_json::json!(false),
                Kind::Int64 | Kind::Uint64 | Kind::Sint64 | Kind::Fixed64 | Kind::Sfixed64 => {
                    serde_json::json!("0")
                }
                _ => serde_json::json!(0),
            }
        };
        if field.containing_oneof().is_none() {
            object.insert(field.json_name().into(), value);
        }
    }
    serde_json::Value::Object(object)
}
pub fn grpc_method(pool: &DescriptorPool, request: &RequestSpec) -> Result<MethodDescriptor> {
    let Protocol::Grpc {
        service, method, ..
    } = &request.protocol
    else {
        anyhow::bail!("Expected gRPC request")
    };
    let service = pool
        .get_service_by_name(service)
        .context("Unknown gRPC service")?;
    service
        .methods()
        .find(|m| m.name() == method)
        .context("Unknown gRPC method")
}
pub fn grpc_message(desc: MessageDescriptor, source: &str) -> Result<DynamicMessage> {
    ensure!(
        source.len() <= 1024 * 1024,
        "gRPC message exceeds 1 MiB JSON limit"
    );
    let mut json = serde_json::Deserializer::from_str(source);
    let message =
        DynamicMessage::deserialize(desc, &mut json).context("Invalid protobuf JSON message")?;
    json.end()?;
    ensure!(
        message.encoded_len() <= 1024 * 1024,
        "gRPC message exceeds 1 MiB protobuf limit"
    );
    Ok(message)
}
pub fn grpc_json(message: &DynamicMessage) -> Result<serde_json::Value> {
    struct BoundedWriter(Vec<u8>);
    impl std::io::Write for BoundedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > 1024 * 1024 {
                return Err(std::io::Error::other("Protobuf JSON exceeds 1 MiB"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut bytes = BoundedWriter(Vec::new());
    message.serialize_with_options(
        &mut serde_json::Serializer::new(&mut bytes),
        &SerializeOptions::new().skip_default_fields(false),
    )?;
    Ok(serde_json::from_slice(&bytes.0)?)
}

/// Pre scripts see/edit the initial gRPC protobuf JSON through pm.request.body.raw.
pub fn prepare_grpc(request: &RequestSpec) -> RequestSpec {
    let mut request = request.clone();
    if let Protocol::Grpc { message_source, .. } = &request.protocol {
        request.body = message_source.clone();
        request.body_kind = "json".into();
        request.method = "POST".into();
    }
    request
}
pub fn reconcile_grpc(request: &mut RequestSpec) -> Result<()> {
    if let Protocol::Grpc { message_source, .. } = &mut request.protocol {
        ensure!(
            request.method == "POST" && request.body_kind == "json",
            "gRPC initial messages require POST with JSON; pre-script method/body-mode changes are unsupported"
        );
        *message_source = request.body.clone();
    }
    Ok(())
}
