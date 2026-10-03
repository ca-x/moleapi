//! Runtime SOAP/WSDL tooling. XML grammar and namespace parsing are delegated to libraries.
use crate::{Pair, Protocol, RequestSpec, Specification};
use anyhow::{Context, Result, bail, ensure};
use roxmltree::{Document, Node, ParsingOptions};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use xmltree::{Element, XMLNode};
use xsd_parser::models::schema::{MaxOccurs, QName, xs};
const WSDL: &str = "http://schemas.xmlsoap.org/wsdl/";
const XSD: &str = "http://www.w3.org/2001/XMLSchema";
const S11: &str = "http://schemas.xmlsoap.org/soap/envelope/";
const S12: &str = "http://www.w3.org/2003/05/soap-envelope";
const B11: &str = "http://schemas.xmlsoap.org/wsdl/soap/";
const B12: &str = "http://schemas.xmlsoap.org/wsdl/soap12/";
pub const MAX_SOAP_SOURCE: usize = 2 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SoapConfig {
    pub version: String,
    #[serde(default)]
    pub service: String,
    #[serde(default)]
    pub port: String,
    #[serde(default)]
    pub operation: String,
    #[serde(default)]
    pub action: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoapSource {
    pub entry_file: String,
    pub files: Vec<SoapFile>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoapFile {
    pub path: String,
    pub content: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SoapFault {
    pub version: String,
    pub code: String,
    pub reason: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
}
#[derive(Serialize)]
pub struct SoapSchema {
    pub services: Vec<SoapService>,
}
#[derive(Serialize)]
pub struct SoapService {
    pub name: String,
    pub ports: Vec<SoapPort>,
}
#[derive(Serialize)]
pub struct SoapPort {
    pub name: String,
    pub binding: String,
    pub version: String,
    pub address: String,
    pub operations: Vec<SoapOperation>,
}
#[derive(Serialize)]
pub struct SoapOperation {
    pub style: String,
    #[serde(rename = "use")]
    pub use_: String,
    pub input_elements: Vec<SoapElement>,
    pub binding_supported: bool,
    pub name: String,
    pub action: String,
    pub template: String,
    pub fields: Vec<SoapField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}
#[derive(Serialize)]
pub struct SoapElement {
    pub namespace: String,
    pub name: String,
}
#[derive(Serialize)]
pub struct SoapField {
    pub name: String,
    pub namespace: String,
    pub type_name: String,
    pub optional: bool,
    pub repeated: bool,
}
pub fn parse_bounded_xml(source: &str) -> Result<Document<'_>> {
    ensure!(source.len() <= crate::MAX_BODY, "XML exceeds 5 MiB");
    let doc = Document::parse_with_options(
        source,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: 20_000,
        },
    )
    .context("Invalid XML (DTD/entities disabled)")?;
    for n in doc.descendants() {
        ensure!(
            n.ancestors().take(66).count() <= 65,
            "XML exceeds depth limit 64"
        );
    }
    Ok(doc)
}
fn ns(version: &str) -> Result<&'static str> {
    match version {
        "1.1" => Ok(S11),
        "1.2" => Ok(S12),
        _ => bail!("SOAP version must be 1.1 or 1.2"),
    }
}
pub fn validate_soap(r: &RequestSpec, draft: bool) -> Result<()> {
    let Protocol::Soap { config } = &r.protocol else {
        return Ok(());
    };
    ns(&config.version)?;
    ensure!(
        config.action.len() <= 4096
            && [&config.service, &config.port, &config.operation]
                .iter()
                .all(|v| v.len() <= 256),
        "SOAP metadata exceeds limit"
    );
    if draft {
        return Ok(());
    }
    ensure!(
        r.method == "POST" && r.body_kind == "text",
        "SOAP requires POST and text XML body"
    );
    let content: Vec<_> = r
        .headers
        .iter()
        .filter(|p| p.enabled && p.key.eq_ignore_ascii_case("content-type"))
        .collect();
    ensure!(content.len() == 1, "SOAP requires one Content-Type header");
    let media: mime::Mime = content[0]
        .value
        .parse()
        .context("Invalid SOAP Content-Type")?;
    let actions: Vec<_> = r
        .headers
        .iter()
        .filter(|p| p.enabled && p.key.eq_ignore_ascii_case("soapaction"))
        .collect();
    if config.version == "1.1" {
        ensure!(
            media.essence_str() == "text/xml" && actions.len() == 1,
            "SOAP1.1 requires text/xml and one SOAPAction"
        );
        let action: String =
            serde_json::from_str(&actions[0].value).context("SOAPAction must be quoted")?;
        ensure!(
            action == config.action,
            "SOAPAction does not match selected action"
        );
    } else {
        ensure!(
            media.essence_str() == "application/soap+xml" && actions.is_empty(),
            "SOAP1.2 requires application/soap+xml without SOAPAction"
        );
        ensure!(
            media.get_param("action").map(|v| v.as_str()) == Some(config.action.as_str()),
            "SOAP1.2 content-type action mismatch"
        );
    }
    let doc = parse_bounded_xml(&r.body)?;
    let root = doc.root_element();
    ensure!(
        root.has_tag_name((ns(&config.version)?, "Envelope")),
        "SOAP Envelope namespace does not match selected version"
    );
    let children: Vec<_> = root.children().filter(Node::is_element).collect();
    ensure!(
        children.len() <= 2
            && children.iter().all(|n| n
                .has_tag_name((ns(&config.version).unwrap_or(""), "Header"))
                || n.has_tag_name((ns(&config.version).unwrap_or(""), "Body"))),
        "Invalid SOAP Envelope children"
    );
    let bodies: Vec<_> = children
        .iter()
        .filter(|n| n.has_tag_name((ns(&config.version).unwrap_or(""), "Body")))
        .collect();
    ensure!(bodies.len() == 1, "SOAP requires exactly one Body");
    ensure!(
        children
            .last()
            .is_some_and(|n| n.has_tag_name((ns(&config.version).unwrap_or(""), "Body"))),
        "SOAP Header must precede Body"
    );
    ensure!(
        bodies[0].children().any(|n| n.is_element()),
        "SOAP Body requires a payload element"
    );
    Ok(())
}
pub fn prepare_soap(r: &RequestSpec) -> Result<RequestSpec> {
    let Protocol::Soap { config } = &r.protocol else {
        return Ok(r.clone());
    };
    validate_soap(r, true)?;
    let mut result = r.clone();
    ensure!(
        config.action.is_ascii() && !config.action.chars().any(char::is_control),
        "SOAP action must be printable ASCII"
    );
    let action = serde_json::to_string(&config.action)?;
    let content = if config.version == "1.1" {
        "text/xml; charset=utf-8".to_owned()
    } else {
        format!("application/soap+xml; charset=utf-8; action={action}")
    };
    for (key, value) in [("content-type", content), ("soapaction", action)] {
        if key == "soapaction" && config.version == "1.2" {
            continue;
        }
        if !result
            .headers
            .iter()
            .any(|p| p.enabled && p.key.eq_ignore_ascii_case(key))
        {
            result.headers.push(Pair {
                id: uuid::Uuid::new_v4().to_string(),
                key: key.into(),
                value,
                enabled: true,
                secret: None,
                local_value: None,
            });
        }
    }
    Ok(result)
}
pub fn soap_fault(source: &str) -> Option<SoapFault> {
    let doc = parse_bounded_xml(source).ok()?;
    let root = doc.root_element();
    let version = if root.has_tag_name((S11, "Envelope")) {
        "1.1"
    } else if root.has_tag_name((S12, "Envelope")) {
        "1.2"
    } else {
        return None;
    };
    let f = root
        .children()
        .find(|n| n.has_tag_name((ns(version).unwrap_or(""), "Body")))?
        .children()
        .find(|n| n.has_tag_name((ns(version).unwrap_or(""), "Fault")))?;
    fn child<'a, 'input>(
        parent: Node<'a, 'input>,
        name: &str,
        namespace: Option<&str>,
    ) -> Option<Node<'a, 'input>> {
        parent.children().find(|n| {
            n.is_element() && n.tag_name().name() == name && n.tag_name().namespace() == namespace
        })
    }
    let text = |node: Option<Node<'_, '_>>| {
        node.map(|n| n.children().filter_map(|n| n.text()).collect::<String>())
            .unwrap_or_default()
    };
    let (code, reason, actor, detail_node) = if version == "1.1" {
        (
            text(child(f, "faultcode", None)),
            text(child(f, "faultstring", None)),
            child(f, "faultactor", None),
            child(f, "detail", None),
        )
    } else {
        (
            text(child(f, "Code", Some(S12)).and_then(|n| child(n, "Value", Some(S12)))),
            text(child(f, "Reason", Some(S12)).and_then(|n| child(n, "Text", Some(S12)))),
            child(f, "Role", Some(S12)),
            child(f, "Detail", Some(S12)),
        )
    };
    Some(SoapFault {
        version: version.into(),
        code,
        reason,
        detail: detail_node
            .map(|n| source[n.range()].to_owned())
            .unwrap_or_default(),
        actor: actor.map(|n| text(Some(n))),
    })
}
fn serialize(e: &Element) -> Result<String> {
    let mut out = Vec::new();
    e.write_with_config(&mut out, xmltree::EmitterConfig::new().perform_indent(true))?;
    Ok(String::from_utf8(out)?)
}
pub fn redact_soap_xml(source: &str) -> Result<String> {
    transform_xml(source, |value| Ok(value.to_owned()), false, true)
}
/// Redact an export clone of a definition document while retaining schema identifiers.
pub fn redact_soap_source_xml(source: &str) -> Result<String> {
    transform_xml(source, |value| Ok(value.to_owned()), true, true)
}

fn path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && path.len() <= 256
            && !path.starts_with('/')
            && !path.contains(['\\', ':'])
            && path.split('/').all(|s| !s.is_empty()
                && s != "."
                && s != ".."
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))),
        "Unsafe XML virtual path"
    );
    Ok(())
}
fn qname(node: Node<'_, '_>, value: &str) -> Result<(String, String)> {
    let q = quick_xml::name::QName(value.as_bytes());
    let local = std::str::from_utf8(q.local_name().as_ref())?.to_owned();
    let prefix = q
        .prefix()
        .map(|p| std::str::from_utf8(p.as_ref()).map(str::to_owned))
        .transpose()?;
    let namespace = node.lookup_namespace_uri(prefix.as_deref()).unwrap_or("");
    ensure!(
        prefix.is_none() || !namespace.is_empty(),
        "Unbound QName prefix"
    );
    Ok((namespace.into(), local))
}
fn xname(q: &QName) -> (String, String) {
    (
        q.namespace().map(ToString::to_string).unwrap_or_default(),
        String::from_utf8_lossy(q.local_name()).into_owned(),
    )
}
struct Types {
    elements: HashMap<(String, String), (xs::ElementType, bool)>,
    complex: HashMap<(String, String), (xs::ComplexBaseType, bool)>,
    simple: HashMap<(String, String), xs::SimpleBaseType>,
}
impl Types {
    fn element(
        &self,
        e: &xs::ElementType,
        namespace: &str,
        qualified: bool,
        global: bool,
        depth: usize,
        fields: &mut Vec<SoapField>,
    ) -> Result<Element> {
        ensure!(
            depth <= 16 && fields.len() < 256,
            "Schema template exceeds depth/field budget (recursive types unsupported)"
        );
        if let Some(reference) = &e.ref_ {
            let key = xname(reference);
            let (found, q) = self
                .elements
                .get(&key)
                .context("Unresolved schema element reference")?;
            let field_start = fields.len();
            let element = self.element(found, &key.0, *q, true, depth + 1, fields)?;
            if let Some(field) = fields.get_mut(field_start) {
                field.optional = e.min_occurs == 0;
                field.repeated = e.max_occurs != MaxOccurs::Bounded(1);
            }
            return Ok(element);
        }
        ensure!(
            e.content.iter().all(|c| matches!(
                c,
                xs::ElementTypeContent::Annotation(_)
                    | xs::ElementTypeContent::SimpleType(_)
                    | xs::ElementTypeContent::ComplexType(_)
            )),
            "Schema alternatives/identity constraints require raw XML"
        );
        ensure!(
            !e.abstract_ && e.substitution_group.is_none(),
            "Abstract/substitution schema elements require raw XML"
        );
        ensure!(
            e.target_namespace.is_none(),
            "XSD1.1 local targetNamespace overrides require raw XML"
        );
        let name = e.name.as_deref().context("Schema element missing name")?;
        let form_default = qualified;
        let qualified = global
            || matches!(e.form, Some(xs::FormChoiceType::Qualified))
            || (qualified && e.form.is_none());
        let element_ns = if qualified { namespace } else { "" };
        let typename = e.type_.as_ref().map(xname);
        fields.push(SoapField {
            name: name.into(),
            namespace: element_ns.into(),
            type_name: typename
                .as_ref()
                .map(|(ns, n)| format!("{{{ns}}}{n}"))
                .unwrap_or_else(|| "anonymous".into()),
            optional: e.min_occurs == 0,
            repeated: e.max_occurs != MaxOccurs::Bounded(1),
        });
        let mut result = Element::new(name);
        let mut namespaces = xmltree::Namespace::empty();
        if !element_ns.is_empty() {
            namespaces.put("m", element_ns);
            result.prefix = Some("m".into());
            result.namespace = Some(element_ns.to_owned());
        }
        result.namespaces = Some(namespaces);
        let inline = e.content.iter().find_map(|c| {
            if let xs::ElementTypeContent::ComplexType(c) = c {
                Some(c)
            } else {
                None
            }
        });
        let complex = if let Some(inline) = inline {
            Some((inline, form_default, namespace))
        } else if let Some(key) = &typename {
            self.complex.get(key).map(|(c, q)| (c, *q, key.0.as_str()))
        } else {
            None
        };
        if let Some((c, q, ns)) = complex {
            self.complex_children(c, ns, q, depth + 1, fields, &mut result)?
        } else {
            if let Some(key) = &typename {
                ensure!(
                    key.0 == XSD || self.simple.contains_key(key),
                    "Unresolved schema type"
                );
            }
            result.children.push(XMLNode::Text(
                e.fixed.clone().or_else(|| e.default.clone()).unwrap_or(
                    if let Some(simple) = e.content.iter().find_map(|c| {
                        if let xs::ElementTypeContent::SimpleType(s) = c {
                            Some(s)
                        } else {
                            None
                        }
                    }) {
                        self.sample_simple(simple, 0)?
                    } else {
                        self.sample_type(typename.as_ref(), 0)?
                    },
                ),
            ));
        }
        Ok(result)
    }
    fn sample_type(&self, key: Option<&(String, String)>, depth: usize) -> Result<String> {
        ensure!(depth <= 16, "Recursive simple type exceeds template budget");
        let Some(key) = key else {
            return Ok(String::new());
        };
        if key.0 == XSD {
            return Ok(match key.1.as_str() {
                "boolean" => "false",
                "positiveInteger" => "1",
                "negativeInteger" => "-1",
                "integer" | "int" | "long" | "short" | "byte" | "unsignedInt" | "unsignedLong"
                | "unsignedShort" | "unsignedByte" | "nonNegativeInteger"
                | "nonPositiveInteger" | "decimal" | "float" | "double" => "0",
                "date" => "1970-01-01",
                "dateTime" => "1970-01-01T00:00:00Z",
                "time" => "00:00:00Z",
                "duration" => "PT0S",
                _ => "",
            }
            .into());
        }
        let simple = self.simple.get(key).context("Unresolved simple type")?;
        self.sample_simple(simple, depth)
    }
    fn sample_simple(&self, simple: &xs::SimpleBaseType, depth: usize) -> Result<String> {
        let restriction = simple
            .content
            .iter()
            .find_map(|c| {
                if let xs::SimpleBaseTypeContent::Restriction(r) = c {
                    Some(r)
                } else {
                    None
                }
            })
            .context("List/union simple types require raw XML")?;
        for c in &restriction.content {
            if let xs::RestrictionContent::Facet(xs::Facet::Enumeration(v)) = c {
                return Ok(v.value.clone());
            }
        }
        self.sample_type(restriction.base.as_ref().map(xname).as_ref(), depth + 1)
    }
    fn complex_children(
        &self,
        c: &xs::ComplexBaseType,
        namespace: &str,
        qualified: bool,
        depth: usize,
        fields: &mut Vec<SoapField>,
        out: &mut Element,
    ) -> Result<()> {
        ensure!(
            !c.abstract_ && c.mixed != Some(true),
            "Abstract/mixed complex types require raw XML"
        );
        for content in &c.content {
            match content {
                xs::ComplexBaseTypeContent::Annotation(_) => {}
                xs::ComplexBaseTypeContent::Sequence(g) | xs::ComplexBaseTypeContent::All(g) => {
                    self.group(g, namespace, qualified, depth, fields, out)?
                }
                _ => bail!(
                    "Schema choice/derivation/attributes/wildcards require raw XML; template unavailable"
                ),
            }
        }
        Ok(())
    }
    fn group(
        &self,
        g: &xs::GroupType,
        namespace: &str,
        qualified: bool,
        depth: usize,
        fields: &mut Vec<SoapField>,
        out: &mut Element,
    ) -> Result<()> {
        ensure!(
            depth <= 16 && g.ref_.is_none(),
            "Schema group exceeds depth budget16 or uses unsupported named group; raw XML required"
        );
        ensure!(
            g.min_occurs <= 32,
            "Schema group minimum exceeds repeat budget32"
        );
        if g.max_occurs == MaxOccurs::Bounded(0) {
            return Ok(());
        }
        let field_start = fields.len();
        for _ in 0..g.min_occurs.max(1) {
            for child in &g.content {
                match child {
                    xs::GroupTypeContent::Element(e) => {
                        ensure!(
                            e.min_occurs <= 32,
                            "Schema element minimum exceeds repeat budget32"
                        );
                        if e.max_occurs != MaxOccurs::Bounded(0) {
                            for _ in 0..e.min_occurs.max(1) {
                                out.children.push(XMLNode::Element(self.element(
                                    e,
                                    namespace,
                                    qualified,
                                    false,
                                    depth + 1,
                                    fields,
                                )?));
                            }
                        }
                    }
                    xs::GroupTypeContent::Sequence(g) | xs::GroupTypeContent::All(g) => {
                        self.group(g, namespace, qualified, depth + 1, fields, out)?
                    }
                    xs::GroupTypeContent::Annotation(_) => {}
                    _ => bail!("Schema choice/group/wildcard requires raw XML"),
                }
            }
        }
        for field in &mut fields[field_start..] {
            field.optional |= g.min_occurs == 0;
            field.repeated |= g.max_occurs != MaxOccurs::Bounded(1);
        }
        Ok(())
    }
}
pub fn soap_schema(spec: &Specification) -> Result<SoapSchema> {
    ensure!(
        spec.kind == "wsdl" && spec.dialect == "wsdl1.1",
        "Expected WSDL1.1 source bundle"
    );
    let source: SoapSource =
        serde_json::from_str(&spec.source).context("Invalid WSDL source bundle JSON")?;
    ensure!(
        !source.files.is_empty()
            && source.files.len() <= 32
            && source.files.iter().map(|f| f.content.len()).sum::<usize>() <= MAX_SOAP_SOURCE,
        "WSDL source exceeds file/byte budget"
    );
    path(&source.entry_file)?;
    let mut paths = HashSet::new();
    let mut docs = Vec::new();
    for f in &source.files {
        path(&f.path)?;
        ensure!(paths.insert(f.path.as_str()), "Duplicate XML virtual path");
        docs.push(parse_bounded_xml(&f.content)?);
    }
    ensure!(
        docs.iter().map(|d| d.descendants().count()).sum::<usize>() <= 20_000,
        "Bundle XML node budget exceeded"
    );
    ensure!(
        docs.iter()
            .flat_map(Document::descendants)
            .filter(|n| n.has_tag_name((XSD, "schema")))
            .count()
            <= 64,
        "Schema count exceeds64"
    );
    let entry = source
        .files
        .iter()
        .position(|f| f.path == source.entry_file)
        .context("WSDL entry file missing")?;
    wsdl::WsDefinitions::from_document(&docs[entry])
        .context("Only WSDL1.1 definitions supported")?;
    let mut imports = 0;
    let mut namespace_work = 0;
    for (i, doc) in docs.iter().enumerate() {
        let root = doc.root_element();
        for node in doc.descendants().filter(Node::is_element) {
            ensure!(
                node.tag_name().name().len() <= 256,
                "XML name metadata exceeds limit"
            );
            for ns in node.namespaces() {
                namespace_work += 1;
                ensure!(
                    namespace_work <= 100_000,
                    "Inherited namespace work budget100000 exceeded"
                );
                ensure!(
                    ns.uri().len() <= 2048 && ns.name().is_none_or(|name| name.len() <= 128),
                    "XML namespace metadata exceeds limit"
                );
            }
            for attr in node.attributes() {
                let limit = match attr.name() {
                    "name" => 256,
                    "targetNamespace" => 2048,
                    "soapAction" | "default" | "fixed" | "value" => 4096,
                    "type" | "ref" | "base" | "element" | "binding" | "message" => 512,
                    "location" | "schemaLocation" => 8192,
                    _ => MAX_SOAP_SOURCE,
                };
                ensure!(
                    attr.value().len() <= limit,
                    "WSDL/schema metadata attribute exceeds limit"
                );
            }
        }
        ensure!(
            root.has_tag_name((WSDL, "definitions")) || root.has_tag_name((XSD, "schema")),
            "Bundle files must be WSDL1.1 or XSD"
        );
        for n in doc.descendants().filter(|n| {
            n.is_element()
                && matches!(
                    n.tag_name().name(),
                    "import" | "include" | "redefine" | "override"
                )
                && (n.tag_name().namespace() == Some(XSD) || n.tag_name().namespace() == Some(WSDL))
        }) {
            imports += 1;
            ensure!(imports <= 64, "Import limit 64 exceeded");
            ensure!(
                !matches!(n.tag_name().name(), "redefine" | "override"),
                "XSD redefine/override unsupported"
            );
            if let Some(location) = n
                .attribute("schemaLocation")
                .or_else(|| n.attribute("location"))
            {
                path(location)?;
                let base = source.files[i]
                    .path
                    .rsplit_once('/')
                    .map(|(d, _)| format!("{d}/{location}"))
                    .unwrap_or_else(|| location.into());
                ensure!(
                    paths.contains(base.as_str()),
                    "Missing import {base}; supply it in virtual bundle (automatic fetching disabled)"
                );
                let imported = docs[source
                    .files
                    .iter()
                    .position(|f| f.path == base)
                    .context("Import missing")?]
                .root_element();
                if n.tag_name().namespace() == Some(WSDL) {
                    ensure!(
                        imported.has_tag_name((WSDL, "definitions"))
                            && imported.attribute("targetNamespace") == n.attribute("namespace"),
                        "WSDL import namespace/type mismatch"
                    );
                } else {
                    ensure!(
                        imported.has_tag_name((XSD, "schema")),
                        "XSD import/include must reference an XSD file"
                    );
                    let expected = if n.tag_name().name() == "include" {
                        n.ancestors()
                            .find(|p| p.has_tag_name((XSD, "schema")))
                            .and_then(|p| p.attribute("targetNamespace"))
                    } else {
                        n.attribute("namespace")
                    };
                    ensure!(
                        imported.attribute("targetNamespace") == expected,
                        "XSD import/include namespace mismatch (chameleon includes unsupported)"
                    );
                }
            }
        }
    }
    let mut parser: xsd_parser::Parser = xsd_parser::Parser::default().resolve_includes(false);
    for doc in &docs {
        let dom = Element::parse(doc.input_text().as_bytes())?;
        fn collect(e: &Element, out: &mut Vec<Element>) {
            if e.name == "schema" && e.namespace.as_deref() == Some(XSD) {
                out.push(e.clone())
            }
            for c in &e.children {
                if let XMLNode::Element(c) = c {
                    collect(c, out)
                }
            }
        }
        let mut schema_doms = Vec::new();
        collect(&dom, &mut schema_doms);
        for (schema, mut element) in doc
            .descendants()
            .filter(|n| n.has_tag_name((XSD, "schema")))
            .zip(schema_doms)
        {
            // Materialize inherited namespace declarations using a mature DOM, preserving originals separately.

            let mut namespaces = xmltree::Namespace::empty();
            for ns in schema.namespaces() {
                namespaces.put(ns.name().unwrap_or(""), ns.uri());
            }
            element.namespaces = Some(namespaces);
            parser = parser
                .add_schema_from_str(&serialize(&element)?)
                .map_err(|e| anyhow::anyhow!("XSD parse: {e}"))?;
        }
    }
    let schemas = parser.finish();
    ensure!(
        schemas
            .schemas()
            .map(|(_, s)| s.schema.content.len())
            .sum::<usize>()
            <= 1024,
        "Global schema component limit1024 exceeded"
    );
    let mut types = Types {
        elements: HashMap::new(),
        complex: HashMap::new(),
        simple: HashMap::new(),
    };
    for (_, info) in schemas.schemas() {
        let namespace = info.schema.target_namespace.clone().unwrap_or_default();
        let qualified = info.schema.element_form_default == xs::FormChoiceType::Qualified;
        for c in &info.schema.content {
            match c {
                xs::SchemaContent::Element(e) => {
                    let key = (
                        namespace.clone(),
                        e.name.clone().context("Global element name missing")?,
                    );
                    ensure!(
                        types.elements.insert(key, (e.clone(), qualified)).is_none(),
                        "Duplicate global schema element"
                    );
                }
                xs::SchemaContent::ComplexType(c) => {
                    let key = (
                        namespace.clone(),
                        c.name.clone().context("Complex type name missing")?,
                    );
                    ensure!(
                        !types.simple.contains_key(&key)
                            && types.complex.insert(key, (c.clone(), qualified)).is_none(),
                        "Duplicate global schema type"
                    );
                }
                xs::SchemaContent::SimpleType(s) => {
                    let key = (
                        namespace.clone(),
                        s.name.clone().context("Simple type name missing")?,
                    );
                    ensure!(
                        !types.complex.contains_key(&key)
                            && types.simple.insert(key, s.clone()).is_none(),
                        "Duplicate global schema type"
                    );
                }
                _ => {}
            }
        }
    }
    let lookup = |kind: &str, key: &(String, String)| -> Result<Node<'_, '_>> {
        let found: Vec<_> = docs
            .iter()
            .flat_map(Document::descendants)
            .filter(|n| {
                n.has_tag_name((WSDL, kind))
                    && n.attribute("name") == Some(key.1.as_str())
                    && n.parent().and_then(|p| p.attribute("targetNamespace"))
                        == Some(key.0.as_str())
            })
            .collect();
        ensure!(
            found.len() == 1,
            "Unresolved or ambiguous WSDL {kind} {}",
            key.1
        );
        Ok(found[0])
    };
    let mut services = Vec::new();
    let mut port_count = 0;
    let mut operation_count = 0;
    let mut template_bytes = 0;
    let mut field_count = 0;
    for doc in &docs {
        if !doc.root_element().has_tag_name((WSDL, "definitions")) {
            continue;
        }
        let def = wsdl::WsDefinitions::from_document(doc)?;
        for service in def.services()? {
            ensure!(
                !services
                    .iter()
                    .any(|s: &SoapService| s.name == service.name().unwrap_or("")),
                "Ambiguous duplicate WSDL service name"
            );
            let mut ports = Vec::new();
            for port in service.ports()? {
                ensure!(
                    !ports
                        .iter()
                        .any(|p: &SoapPort| p.name == port.name().unwrap_or("")),
                    "Duplicate WSDL port name"
                );
                port_count += 1;
                ensure!(port_count <= 64, "SOAP port budget64 exceeded");
                let binding_key = qname(
                    port.node(),
                    port.node()
                        .attribute("binding")
                        .context("Port binding missing")?,
                )?;
                let binding = lookup("binding", &binding_key)?;
                let sb = binding
                    .children()
                    .find(|n| n.has_tag_name((B11, "binding")) || n.has_tag_name((B12, "binding")))
                    .context("Non-SOAP WSDL binding unsupported")?;
                ensure!(
                    sb.attribute("transport") == Some("http://schemas.xmlsoap.org/soap/http"),
                    "Only SOAP HTTP binding supported"
                );
                let bns = sb.tag_name().namespace().unwrap_or(B11);
                let version = if bns == B12 { "1.2" } else { "1.1" };
                let address = port
                    .node()
                    .children()
                    .find(|n| n.has_tag_name((bns, "address")))
                    .and_then(|n| n.attribute("location"))
                    .context("SOAP port address missing")?
                    .to_owned();
                let port_type = lookup(
                    "portType",
                    &qname(
                        binding,
                        binding.attribute("type").context("Binding type missing")?,
                    )?,
                )?;
                let mut operations = Vec::new();
                for op in binding
                    .children()
                    .filter(|n| n.has_tag_name((WSDL, "operation")))
                {
                    operation_count += 1;
                    ensure!(operation_count <= 128, "WSDL operation budget128 exceeded");
                    let name = op
                        .attribute("name")
                        .context("Operation name missing")?
                        .to_owned();
                    ensure!(
                        !operations.iter().any(|o: &SoapOperation| o.name == name),
                        "Overloaded/duplicate WSDL binding operation names unsupported"
                    );
                    let sop = op
                        .children()
                        .find(|n| n.has_tag_name((bns, "operation")))
                        .context("SOAP operation missing")?;
                    let action = sop.attribute("soapAction").unwrap_or("").to_owned();
                    let style = sop
                        .attribute("style")
                        .or_else(|| sb.attribute("style"))
                        .unwrap_or("document")
                        .to_owned();
                    let use_ = op
                        .children()
                        .find(|n| n.has_tag_name((WSDL, "input")))
                        .and_then(|n| n.children().find(|n| n.has_tag_name((bns, "body"))))
                        .and_then(|n| n.attribute("use"))
                        .unwrap_or("")
                        .to_owned();
                    let binding_supported = style == "document"
                        && use_ == "literal"
                        && op
                            .children()
                            .find(|n| n.has_tag_name((WSDL, "input")))
                            .and_then(|n| n.children().find(|n| n.has_tag_name((bns, "body"))))
                            .is_some_and(|n| {
                                n.attribute("encodingStyle").is_none()
                                    && n.attribute("parts").is_none()
                            });
                    let mut input_elements = Vec::new();
                    let mut fields = Vec::new();
                    let template = (|| -> Result<String> {
                        let matches: Vec<_> = port_type
                            .children()
                            .filter(|n| {
                                n.has_tag_name((WSDL, "operation"))
                                    && n.attribute("name") == Some(name.as_str())
                            })
                            .collect();
                        ensure!(
                            matches.len() == 1,
                            "Missing or overloaded WSDL port operation unsupported"
                        );
                        let operation = matches[0];
                        let input = operation
                            .children()
                            .find(|n| n.has_tag_name((WSDL, "input")))
                            .context("Operation input missing")?;
                        let message = lookup(
                            "message",
                            &qname(
                                input,
                                input
                                    .attribute("message")
                                    .context("Message reference missing")?,
                            )?,
                        )?;
                        let parts = message
                            .children()
                            .filter(|n| n.has_tag_name((WSDL, "part")))
                            .map(|part| {
                                qname(
                                    part,
                                    part.attribute("element")
                                        .context("Type-based message parts require raw XML")?,
                                )
                            })
                            .collect::<Result<Vec<_>>>()?;
                        input_elements = parts
                            .iter()
                            .map(|(namespace, name)| SoapElement {
                                namespace: namespace.clone(),
                                name: name.clone(),
                            })
                            .collect();
                        ensure!(
                            sop.attribute("style")
                                .or_else(|| sb.attribute("style"))
                                .unwrap_or("document")
                                == "document",
                            "RPC style requires raw XML; schema template unavailable"
                        );
                        let input = op
                            .children()
                            .find(|n| n.has_tag_name((WSDL, "input")))
                            .context("Binding input missing")?;
                        let body = input
                            .children()
                            .find(|n| n.has_tag_name((bns, "body")))
                            .context("SOAP body binding missing")?;
                        ensure!(
                            body.attribute("use") == Some("literal")
                                && body.attribute("encodingStyle").is_none()
                                && body.attribute("parts").is_none(),
                            "Encoded/part-filtered SOAP binding unsupported"
                        );
                        ensure!(
                            !input.children().any(|n| n.has_tag_name((bns, "header"))),
                            "WSDL SOAP headers require raw XML"
                        );
                        let mut envelope = Element::new("Envelope");
                        envelope.prefix = Some("soap".into());
                        let mut namespaces = xmltree::Namespace::empty();
                        namespaces.put("soap", ns(version)?);
                        envelope.namespaces = Some(namespaces);
                        let mut body = Element::new("Body");
                        body.prefix = Some("soap".into());
                        for key in parts {
                            let (e, q) = types
                                .elements
                                .get(&key)
                                .context("Message schema element missing")?;
                            body.children.push(XMLNode::Element(types.element(
                                e,
                                &key.0,
                                *q,
                                true,
                                0,
                                &mut fields,
                            )?));
                        }
                        ensure!(!body.children.is_empty(), "SOAP message has no input parts");
                        envelope.children.push(XMLNode::Element(body));
                        let text = serialize(&envelope)?;
                        parse_bounded_xml(&text)?;
                        ensure!(text.len() <= 1024 * 1024, "Template exceeds 1MiB");
                        Ok(text)
                    })();
                    let (template, error) = match template {
                        Ok(t) => (t, None),
                        Err(e) => {
                            fields.clear();
                            (String::new(), Some(e.to_string()))
                        }
                    };
                    template_bytes += template.len();
                    field_count += fields.len();
                    ensure!(
                        template_bytes <= MAX_SOAP_SOURCE && field_count <= 1024,
                        "Aggregate SOAP template byte/field budget exceeded"
                    );
                    operations.push(SoapOperation {
                        style,
                        use_,
                        input_elements,
                        binding_supported,
                        name,
                        action,
                        template,
                        fields,
                        error,
                    });
                }
                ports.push(SoapPort {
                    name: port.name()?.into(),
                    binding: format!("{{{}}}{}", binding_key.0, binding_key.1),
                    version: version.into(),
                    address,
                    operations,
                });
            }
            services.push(SoapService {
                name: service.name()?.into(),
                ports,
            });
            ensure!(services.len() <= 64, "Service limit exceeded");
        }
    }
    ensure!(!services.is_empty(), "WSDL bundle has no services");
    Ok(SoapSchema { services })
}
/// Interpolate XML data nodes through the DOM, so scoped values cannot create XML markup.
pub fn resolve_soap_xml(
    source: &str,
    resolve: impl FnMut(&str) -> Result<String>,
) -> Result<String> {
    transform_xml(source, resolve, false, false)
}

fn transform_xml(
    source: &str,
    mut resolve: impl FnMut(&str) -> Result<String>,
    definitions: bool,
    redact: bool,
) -> Result<String> {
    use quick_xml::{
        Reader, Writer,
        events::{BytesStart, BytesText, Event},
    };
    let document = parse_bounded_xml(source)?;
    // Nodes are supplied by roxmltree, in the same preorder as quick-xml events.
    let mut nodes = document.descendants().filter(Node::is_element);
    let mut reader = Reader::from_str(source);
    let mut writer = Writer::new(Vec::new());
    let mut hidden = Vec::new();
    loop {
        let event = reader.read_event()?;
        match event {
            Event::Start(ref start) | Event::Empty(ref start) => {
                let node = nodes.next().context("XML reader node mismatch")?;
                let schema = definitions
                    && node
                        .tag_name()
                        .namespace()
                        .is_some_and(|ns| matches!(ns, XSD | WSDL | B11 | B12));
                let sensitive =
                    redact && !schema && crate::sensitive_query_key(node.tag_name().name());
                let sensitive_default = redact
                    && definitions
                    && node.tag_name().namespace() == Some(XSD)
                    && matches!(node.tag_name().name(), "element" | "attribute")
                    && node
                        .attribute("name")
                        .is_some_and(crate::sensitive_query_key);
                let outer = hidden.last().copied().unwrap_or(false);
                let name = std::str::from_utf8(start.name().as_ref())?.to_owned();
                let mut transformed = BytesStart::new(name);
                for attr in start.attributes() {
                    let attr = attr?;
                    let key = std::str::from_utf8(attr.key.as_ref())?;
                    let local = std::str::from_utf8(attr.key.local_name().as_ref())?.to_owned();
                    let value = attr.decode_and_unescape_value(reader.decoder())?;
                    let xmlns =
                        key == "xmlns" || attr.key.prefix().is_some_and(|p| p.as_ref() == b"xmlns");
                    let value = if xmlns {
                        ensure!(
                            !value.contains("{{"),
                            "XML namespace URIs cannot be templates"
                        );
                        value.into_owned()
                    } else if redact
                        && ((sensitive_default && matches!(local.as_str(), "default" | "fixed"))
                            || crate::sensitive_query_key(&local))
                    {
                        "[REDACTED]".into()
                    } else if redact
                        && definitions
                        && matches!(
                            local.as_str(),
                            "location" | "schemaLocation" | "href" | "url" | "endpoint"
                        )
                    {
                        redact_xml_url(&value)
                    } else {
                        resolve(&value)?
                    };
                    transformed.push_attribute((key, value.as_str()));
                }
                if matches!(event, Event::Start(_)) {
                    hidden.push(outer || sensitive);
                    if !outer {
                        writer.write_event(Event::Start(transformed))?;
                        if sensitive {
                            writer.write_event(Event::Text(BytesText::new("[REDACTED]")))?;
                        }
                    }
                } else if !outer {
                    writer.write_event(Event::Empty(transformed))?;
                }
            }
            Event::End(end) => {
                hidden.pop().context("XML element stack mismatch")?;
                if !hidden.last().copied().unwrap_or(false) {
                    writer.write_event(Event::End(end))?;
                }
            }
            Event::Text(text) if !hidden.last().copied().unwrap_or(false) => {
                let decoded = text.xml_content()?;
                let value = quick_xml::escape::unescape(&decoded)?;
                let value = resolve(&value)?;
                writer.write_event(Event::Text(BytesText::new(&value)))?;
            }
            Event::CData(text) if !hidden.last().copied().unwrap_or(false) => {
                let decoded = text.decode()?;
                let value = resolve(&decoded)?;
                writer.write_event(Event::Text(BytesText::new(&value)))?;
            }
            Event::DocType(_) => bail!("DTD is disabled"),
            Event::Eof => break,
            event if !hidden.last().copied().unwrap_or(false) => writer.write_event(event)?,
            _ => {}
        }
        ensure!(
            writer.get_ref().len() <= crate::MAX_BODY,
            "Resolved/redacted XML exceeds size budget"
        );
    }
    let text = String::from_utf8(writer.into_inner())?;
    parse_bounded_xml(&text)?;
    Ok(text)
}
fn redact_xml_url(value: &str) -> String {
    let Ok(mut url) = url::Url::parse(value) else {
        return value.into();
    };
    if !matches!(url.scheme(), "http" | "https") {
        return value.into();
    }
    let _ = url.set_username("");
    let _ = url.set_password(None);
    crate::redact_url(url.as_str(), None)
}

/// Shared XML escape form used when screening persistent history/export metadata.
pub fn escape_xml_value(value: &str) -> String {
    quick_xml::escape::escape(value).into_owned()
}
/// Decoded XML data for bounded privacy screening, including numeric entity spellings.
/// This is supplementary data; it never rewrites the canonical XML/source document.
pub fn soap_xml_data_values(source: &str) -> Result<Vec<String>> {
    let document = parse_bounded_xml(source)?;
    let mut values = std::collections::BTreeSet::new();
    let text = document
        .descendants()
        .filter(Node::is_text)
        .filter_map(|n| n.text())
        .collect::<String>();
    values.insert(text);
    for node in document.descendants().filter(Node::is_element) {
        for attr in node.attributes() {
            values.insert(attr.value().to_owned());
        }
    }
    // Enumerate declaration attributes once. roxmltree namespaces() repeats every
    // inherited namespace on each child and can amplify a small input quadratically.
    let mut reader = quick_xml::Reader::from_str(source);
    loop {
        match reader.read_event()? {
            quick_xml::events::Event::Start(start) | quick_xml::events::Event::Empty(start) => {
                for attr in start.attributes() {
                    let attr = attr?;
                    if attr.key.as_ref() == b"xmlns"
                        || attr.key.prefix().is_some_and(|p| p.as_ref() == b"xmlns")
                    {
                        values.insert(
                            attr.decode_and_unescape_value(reader.decoder())?
                                .into_owned(),
                        );
                    }
                }
            }
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    Ok(values.into_iter().collect())
}
