//! SHA2 extension points of the OAuth SDK. RustCrypto performs every MAC/signature.
use base64::{Engine, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use oauth1_request::signature_method::{Sign, SignatureMethod};
use rsa::{Pkcs1v15Sign, RsaPrivateKey};
use sha2::{Digest, Sha256, Sha512};
use std::{
    fmt::Display,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
#[derive(Clone)]
pub(super) struct Method {
    name: &'static str,
    key: Option<RsaPrivateKey>,
    pub failed: Arc<AtomicBool>,
}
impl Method {
    pub fn new(name: &str, key: Option<RsaPrivateKey>) -> Self {
        Self {
            name: match name {
                "HMAC-SHA256" => "HMAC-SHA256",
                "HMAC-SHA512" => "HMAC-SHA512",
                "RSA-SHA256" => "RSA-SHA256",
                "RSA-SHA512" => "RSA-SHA512",
                _ => unreachable!(),
            },
            key,
            failed: Arc::default(),
        }
    }
}
pub(super) struct Signer {
    method: Method,
    secret: String,
    base: String,
}
impl SignatureMethod for Method {
    type Sign = Signer;
    fn sign_with(self, secret: &str, token: Option<&str>) -> Signer {
        Signer {
            method: self,
            secret: format!(
                "{}&{}",
                super::encode(secret),
                super::encode(token.unwrap_or(""))
            ),
            base: String::new(),
        }
    }
}
impl Sign for Signer {
    type Signature = String;
    fn get_signature_method_name(&self) -> &'static str {
        self.method.name
    }
    fn request_method(&mut self, method: &str) {
        self.base.push_str(method);
        self.base.push('&');
    }
    fn uri<T: Display>(&mut self, uri: T) {
        self.base.push_str(&format!("{uri}&"));
    }
    fn parameter<V: Display>(&mut self, key: &str, value: V) {
        self.base.push_str(&format!("{key}%3D{value}"));
    }
    fn delimiter(&mut self) {
        self.base.push_str("%26");
    }
    fn end(self) -> String {
        let signature = match self.method.name {
            "HMAC-SHA256" => {
                let mut mac = Hmac::<Sha256>::new_from_slice(self.secret.as_bytes())
                    .expect("HMAC accepts all key lengths");
                mac.update(self.base.as_bytes());
                Ok(mac.finalize().into_bytes().to_vec())
            }
            "HMAC-SHA512" => {
                let mut mac = Hmac::<Sha512>::new_from_slice(self.secret.as_bytes())
                    .expect("HMAC accepts all key lengths");
                mac.update(self.base.as_bytes());
                Ok(mac.finalize().into_bytes().to_vec())
            }
            "RSA-SHA256" => self
                .method
                .key
                .as_ref()
                .expect("validated RSA method")
                .sign(
                    Pkcs1v15Sign::new::<Sha256>(),
                    &Sha256::digest(self.base.as_bytes()),
                ),
            "RSA-SHA512" => self
                .method
                .key
                .as_ref()
                .expect("validated RSA method")
                .sign(
                    Pkcs1v15Sign::new::<Sha512>(),
                    &Sha512::digest(self.base.as_bytes()),
                ),
            _ => unreachable!(),
        };
        match signature {
            Ok(bytes) => super::encode(&STANDARD.encode(bytes)),
            Err(_) => {
                self.method.failed.store(true, Ordering::Relaxed);
                String::new()
            }
        }
    }
}

/// SDK PLAINTEXT returns the encoded signing key, while Authorizer expects the
/// entire signature value to be encoded once more (including its '&' separator).
#[derive(Clone)]
pub(super) struct Plaintext;
pub(super) struct PlainSign(oauth1_request::signature_method::plaintext::PlaintextSign<String>);
impl SignatureMethod for Plaintext {
    type Sign = PlainSign;
    fn sign_with(self, secret: &str, token: Option<&str>) -> PlainSign {
        PlainSign(oauth1_request::signature_method::PLAINTEXT.sign_with(secret, token))
    }
}
impl Sign for PlainSign {
    type Signature = String;
    fn get_signature_method_name(&self) -> &'static str {
        "PLAINTEXT"
    }
    fn request_method(&mut self, method: &str) {
        self.0.request_method(method);
    }
    fn uri<T: Display>(&mut self, uri: T) {
        self.0.uri(uri);
    }
    fn parameter<V: Display>(&mut self, key: &str, value: V) {
        self.0.parameter(key, value);
    }
    fn delimiter(&mut self) {
        self.0.delimiter();
    }
    fn end(self) -> String {
        super::encode(&self.0.end())
    }
}

#[derive(Clone)]
pub(super) struct Capture<M> {
    pub method: M,
    pub values: Arc<std::sync::Mutex<Vec<String>>>,
}
pub(super) struct Captured<S> {
    inner: S,
    values: Arc<std::sync::Mutex<Vec<String>>>,
}
impl<M: SignatureMethod> SignatureMethod for Capture<M> {
    type Sign = Captured<M::Sign>;
    fn sign_with(self, secret: &str, token: Option<&str>) -> Self::Sign {
        Captured {
            inner: self.method.sign_with(secret, token),
            values: self.values,
        }
    }
}
impl<S: Sign> Sign for Captured<S> {
    type Signature = String;
    fn get_signature_method_name(&self) -> &'static str {
        self.inner.get_signature_method_name()
    }
    fn request_method(&mut self, method: &str) {
        self.inner.request_method(method);
    }
    fn uri<T: Display>(&mut self, uri: T) {
        self.inner.uri(uri);
    }
    fn parameter<V: Display>(&mut self, key: &str, value: V) {
        self.inner.parameter(key, value);
    }
    fn delimiter(&mut self) {
        self.inner.delimiter();
    }
    fn use_nonce(&self) -> bool {
        self.inner.use_nonce()
    }
    fn use_timestamp(&self) -> bool {
        self.inner.use_timestamp()
    }
    fn end(self) -> String {
        let signature = self.inner.end().to_string();
        let mut values = self.values.lock().unwrap();
        values.push(signature.clone());
        if let Ok(decoded) = percent_encoding::percent_decode_str(&signature).decode_utf8() {
            values.push(decoded.into_owned());
        }
        signature
    }
}
