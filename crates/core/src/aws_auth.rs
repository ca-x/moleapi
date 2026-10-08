//! Request signing is owned by the official AWS SDK; source contains no ambient credentials.
use crate::AuthLocation;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AwsAuth {
    pub access_key: String,
    pub secret_key: String,
    pub session_token: String,
    pub region: String,
    pub service: String,
    pub location: AuthLocation,
    pub expires_seconds: u64,
    pub unsigned_payload: bool,
}
impl Default for AwsAuth {
    fn default() -> Self {
        Self {
            access_key: String::new(),
            secret_key: String::new(),
            session_token: String::new(),
            region: "us-east-1".into(),
            service: "execute-api".into(),
            location: AuthLocation::Header,
            expires_seconds: 900,
            unsigned_payload: false,
        }
    }
}
pub(crate) fn validate_aws_fields(config: &AwsAuth) -> Result<()> {
    ensure!(
        config.access_key.len() <= 4096
            && config.secret_key.len() <= 65536
            && config.session_token.len() <= 65536,
        "AWS credentials exceed limits"
    );
    ensure!(
        config.region.len() <= 128 && config.service.len() <= 128,
        "AWS region/service exceeds limits"
    );
    Ok(())
}
pub fn validate_aws(config: &AwsAuth, templates: bool) -> Result<()> {
    validate_aws_fields(config)?;
    ensure!(
        (1..=604800).contains(&config.expires_seconds),
        "AWS presign expiry must be1..604800 seconds"
    );
    for value in [&config.region, &config.service] {
        ensure!(
            !value.is_empty() && value.len() <= 128,
            "AWS region/service is missing or exceeds limits"
        );
        if !(templates && value.contains("{{")) {
            ensure!(
                value
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
                "Invalid AWS region/service"
            );
        }
    }
    if !templates {
        ensure!(
            !config.access_key.is_empty() && !config.secret_key.is_empty(),
            "AWS signing credentials are missing; supply them again"
        );
        ensure!(
            [
                &config.access_key,
                &config.secret_key,
                &config.session_token
            ]
            .iter()
            .all(|s| !s.chars().any(char::is_control)),
            "Invalid AWS credentials"
        );
    }
    Ok(())
}
pub fn sign_aws_request(
    config: &AwsAuth,
    request: &mut reqwest::Request,
    time: std::time::SystemTime,
) -> Result<Vec<String>> {
    use aws_sigv4::http_request::{
        PayloadChecksumKind, PercentEncodingMode, SignableBody, SignableRequest, SignatureLocation,
        SigningSettings, UriPathNormalizationMode, sign,
    };
    validate_aws(config, false)?;
    ensure!(
        !request.url().query_pairs().any(|(key, _)| [
            "x-amz-algorithm",
            "x-amz-credential",
            "x-amz-date",
            "x-amz-expires",
            "x-amz-signedheaders",
            "x-amz-signature",
            "x-amz-security-token"
        ]
        .contains(&key.to_ascii_lowercase().as_str())),
        "AWS query already contains signing parameters"
    );
    for name in [
        "authorization",
        "x-amz-date",
        "x-amz-security-token",
        "x-amz-content-sha256",
    ] {
        ensure!(
            !request.headers().contains_key(name),
            "AWS request already contains a signing header"
        );
    }
    let body = request
        .body()
        .map(|body| {
            body.as_bytes()
                .context("AWS signing requires materialized body bytes")
        })
        .transpose()?
        .unwrap_or(&[]);
    let identity = aws_credential_types::Credentials::new(
        &config.access_key,
        &config.secret_key,
        if config.session_token.is_empty() {
            None
        } else {
            Some(config.session_token.clone())
        },
        None,
        "MoleAPI source",
    )
    .into();
    let mut settings = SigningSettings::default();
    if config.service == "s3" {
        settings.percent_encoding_mode = PercentEncodingMode::Single;
        settings.uri_path_normalization_mode = UriPathNormalizationMode::Disabled;
        settings.payload_checksum_kind = PayloadChecksumKind::XAmzSha256;
    }
    settings.signature_location = if config.location == AuthLocation::Query {
        SignatureLocation::QueryParams
    } else {
        SignatureLocation::Headers
    };
    if config.location == AuthLocation::Query {
        settings.expires_in = Some(std::time::Duration::from_secs(config.expires_seconds));
    }
    let params = aws_sigv4::sign::v4::SigningParams::builder()
        .identity(&identity)
        .region(&config.region)
        .name(&config.service)
        .time(time)
        .settings(settings)
        .build()
        .context("Invalid AWS signing configuration")?
        .into();
    let headers = request
        .headers()
        .iter()
        .map(|(key, value)| Ok((key.as_str(), value.to_str()?)))
        .collect::<Result<Vec<_>>>()?;
    let signable = SignableRequest::new(
        request.method().as_str(),
        request.url().as_str(),
        headers.into_iter(),
        if config.unsigned_payload
            || config.service == "s3" && config.location == AuthLocation::Query
        {
            SignableBody::UnsignedPayload
        } else {
            SignableBody::Bytes(body)
        },
    )
    .context("AWS request cannot be signed")?;
    let (instructions, signature) = sign(signable, &params)
        .context("AWS signing failed")?
        .into_parts();
    let mut private = vec![signature.to_string()];
    for (name, value) in instructions.headers() {
        if name.eq_ignore_ascii_case("authorization")
            || name.eq_ignore_ascii_case("x-amz-security-token")
        {
            private.push(value.into());
        }
    }
    // The SDK applies its exact query encoding; preserve these bytes at the wire.
    let mut bridge = http::Request::builder()
        .method(request.method())
        .uri(request.url().as_str())
        .body(())?;
    *bridge.headers_mut() = request.headers().clone();
    instructions.apply_to_request_http1x(&mut bridge);
    *request.url_mut() = url::Url::parse(&bridge.uri().to_string())?;
    *request.headers_mut() = bridge.headers().clone();
    Ok(private)
}
