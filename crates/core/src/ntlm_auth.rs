//! NTLMv2 comes from the SSPI SDK; HTTP connection affinity belongs to Hyper.
mod network;
use anyhow::{Context, Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use sspi::{
    AuthIdentity, AuthIdentityBuffers, BufferType, ClientRequestFlags, CredentialUse,
    DataRepresentation, Ntlm, Secret, SecurityBuffer, SecurityStatus, Sspi, SspiImpl, Username,
};
mod connection;
pub(crate) use connection::Connection;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct NtlmAuth {
    pub domain: String,
    pub workstation: String,
    pub channel_binding: bool,
}
impl Default for NtlmAuth {
    fn default() -> Self {
        Self {
            domain: String::new(),
            workstation: String::new(),
            channel_binding: true,
        }
    }
}
pub(crate) fn validate_ntlm_fields(c: &NtlmAuth) -> Result<()> {
    ensure!(
        c.domain.len() <= 256 && c.workstation.len() <= 256,
        "NTLM domain/workstation exceed limits"
    );
    Ok(())
}
pub fn validate_ntlm(auth: &crate::Auth, templates: bool) -> Result<()> {
    let default = NtlmAuth::default();
    let c = auth.ntlm.as_deref().unwrap_or(&default);
    validate_ntlm_fields(c)?;
    if !templates {
        ensure!(!auth.username.is_empty(), "NTLM username is missing");
        ensure!(
            ![&auth.username, &c.domain, &c.workstation]
                .iter()
                .any(|s| s.chars().any(char::is_control)),
            "NTLM identity contains control characters"
        );
        Username::new(
            &auth.username,
            (!c.domain.is_empty()).then_some(c.domain.as_str()),
        )
        .map_err(|_| anyhow::anyhow!("Invalid NTLM username/domain"))?;
    }
    Ok(())
}
pub(crate) struct Handshake {
    ntlm: Ntlm,
    credentials: Option<AuthIdentityBuffers>,
    target: String,
    stage: u8,
}
impl Handshake {
    pub fn new(auth: &crate::Auth, host: &str, cbt: Option<&[u8]>) -> Result<Self> {
        validate_ntlm(auth, false)?;
        let default = NtlmAuth::default();
        let c = auth.ntlm.as_deref().unwrap_or(&default);
        let identity = AuthIdentity {
            username: Username::new(
                &auth.username,
                (!c.domain.is_empty()).then_some(c.domain.as_str()),
            )
            .map_err(|_| anyhow::anyhow!("Invalid NTLM username/domain"))?,
            password: Secret::from(auth.password.clone()),
        };
        let mut ntlm = Ntlm::with_config(sspi::ntlm::NtlmConfig {
            client_computer_name: (!c.workstation.is_empty()).then(|| c.workstation.clone()),
        });
        if c.channel_binding
            && let Some(cbt) = cbt
        {
            ntlm.set_channel_bindings(cbt);
        }
        let acquired = ntlm
            .acquire_credentials_handle()
            .with_credential_use(CredentialUse::Outbound)
            .with_auth_data(&identity)
            .execute(&mut ntlm)
            .map_err(|_| anyhow::anyhow!("NTLM credentials cannot be prepared"))?;
        Ok(Self {
            ntlm,
            credentials: acquired.credentials_handle,
            target: format!("HTTP/{host}"),
            stage: 0,
        })
    }
    pub fn next(&mut self, challenge: Option<&[u8]>) -> Result<String> {
        ensure!(
            self.stage < 2,
            "NTLM authentication exceeded two challenge rounds"
        );
        if self.stage == 1 {
            ensure!(
                challenge.is_some_and(|t| !t.is_empty()),
                "NTLM server did not send a Type2 challenge"
            );
        }
        let mut input = [SecurityBuffer::new(
            challenge.unwrap_or_default().to_vec(),
            BufferType::Token,
        )];
        let mut output = [SecurityBuffer::new(Vec::new(), BufferType::Token)];
        let mut builder = self
            .ntlm
            .initialize_security_context()
            .with_credentials_handle(&mut self.credentials)
            .with_context_requirements(ClientRequestFlags::CONNECTION)
            .with_target_data_representation(DataRepresentation::Native)
            .with_target_name(&self.target)
            .with_output(&mut output);
        if self.stage > 0 {
            builder.input = Some(&mut input);
        }
        let result = self
            .ntlm
            .initialize_security_context_impl(&mut builder)
            .map_err(|_| anyhow::anyhow!("NTLM challenge negotiation failed"))?
            .resolve_to_result()
            .map_err(|_| anyhow::anyhow!("NTLM challenge negotiation failed"))?;
        ensure!(
            matches!(
                result.status,
                SecurityStatus::Ok | SecurityStatus::ContinueNeeded
            ),
            "Unsupported NTLM negotiation status"
        );
        ensure!(
            !output[0].buffer.is_empty() && output[0].buffer.len() <= 65536,
            "NTLM generated token exceeds limit"
        );
        self.stage += 1;
        Ok(format!("NTLM {}", STANDARD.encode(&output[0].buffer)))
    }
    pub fn complete(&self) -> bool {
        self.stage >= 2
    }
}
pub(crate) fn challenge(headers: &reqwest::header::HeaderMap) -> Result<Option<Option<Vec<u8>>>> {
    use hyperx::header::{Header, Raw};
    use www_authenticate::{RawChallenge, WwwAuthenticate};
    let mut bare = false;
    for value in headers.get_all("www-authenticate").iter().take(16) {
        let value = value.to_str().context("Invalid NTLM challenge header")?;
        ensure!(value.len() <= 65536, "NTLM challenge header exceeds limit");
        if value.trim().eq_ignore_ascii_case("ntlm") {
            bare = true;
            continue;
        }
        if let Ok(parsed) = WwwAuthenticate::parse_header(&Raw::from(value))
            && let Some(entries) = parsed.get_raw("NTLM")
        {
            for entry in entries {
                match entry {
                    RawChallenge::Token68(token) => {
                        let decoded = STANDARD
                            .decode(token)
                            .context("Invalid NTLM challenge encoding")?;
                        ensure!(decoded.len() <= 49152, "NTLM challenge exceeds limit");
                        return Ok(Some(Some(decoded)));
                    }
                    RawChallenge::Fields(_) => bare = true,
                }
            }
        }
        for item in http_auth::ChallengeParser::new(value) {
            let Ok(item) = item else {
                break;
            };
            if item.scheme.eq_ignore_ascii_case("ntlm") && item.params.is_empty() {
                bare = true;
            }
        }
    }
    Ok(bare.then_some(None))
}
pub(crate) async fn drain(response: reqwest::Response) -> Result<()> {
    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();
    let mut count = 0;
    while let Some(chunk) = stream.next().await {
        count += chunk
            .context("Reading NTLM challenge response failed")?
            .len();
        ensure!(
            count <= crate::MAX_BODY,
            "NTLM challenge response exceeds 5 MiB"
        );
    }
    Ok(())
}
