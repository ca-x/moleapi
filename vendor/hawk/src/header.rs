use crate::b64;
use crate::error::*;
use crate::mac::Mac;
use base64::Engine;
use std::fmt;
// The quoted-string crate owns scan/escape/unescape; HTTP character rules are
// supplied by the pinned media-type utility, not an application parser.
#[derive(Clone, Debug)]
struct HttpQuoted;
impl quoted_string::spec::GeneralQSSpec for HttpQuoted {
    type Quoting = media_type_impl_utils::quoted_string::NormalUtf8Quoting;
    type Parsing = HttpQuotedParsing;
}
// Combine the library's HTTP normal-state rules with its VCHAR quoted-pair
// rules: the upstream obs profile accidentally excludes escaped DQUOTE/\\.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HttpQuotedParsing;
impl quoted_string::spec::ParsingImpl for HttpQuotedParsing {
    fn can_be_quoted(point: quoted_string::spec::PartialCodePoint) -> bool {
        point.as_u8()>0x7f || <media_type_impl_utils::quoted_string::NormalParsingImpl as quoted_string::spec::ParsingImpl>::can_be_quoted(point)
    }
    fn handle_normal_state(
        point: quoted_string::spec::PartialCodePoint,
    ) -> std::result::Result<
        (quoted_string::spec::State<Self>, bool),
        quoted_string::error::CoreError,
    > {
        <media_type_impl_utils::quoted_string::HttpObsParsingImpl as quoted_string::spec::ParsingImpl>::handle_normal_state(point).map(|(_,emit)|(quoted_string::spec::State::Normal,emit))
    }
}
fn quote_header(value: &str) -> Result<String> {
    quoted_string::quote::<HttpQuoted>(value)
        .map_err(|_| Error::HeaderParseError("Invalid Hawk quoted field".into()))
}
use std::str::FromStr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Representation of a Hawk `Authorization` header value (the part following "Hawk ").
///
/// Headers can be derived from strings using the `FromStr` trait, and formatted into a
/// string using the `fmt_header` method.
///
/// All fields are optional, although for specific purposes some fields must be present.
#[derive(Clone, PartialEq, Debug)]
pub struct Header {
    pub id: Option<String>,
    pub ts: Option<SystemTime>,
    pub nonce: Option<String>,
    pub mac: Option<Mac>,
    pub ext: Option<String>,
    pub hash: Option<Vec<u8>>,
    pub app: Option<String>,
    pub dlg: Option<String>,
}

impl Header {
    /// Create a new Header with the full set of Hawk fields.
    ///
    /// This is a low-level function. Headers are more often created from Requests or Responses.
    ///
    /// Note that none of the string-formatted header components can contain the character `\"`.
    pub fn new<S>(
        id: Option<S>,
        ts: Option<SystemTime>,
        nonce: Option<S>,
        mac: Option<Mac>,
        ext: Option<S>,
        hash: Option<Vec<u8>>,
        app: Option<S>,
        dlg: Option<S>,
    ) -> Result<Header>
    where
        S: Into<String>,
    {
        Ok(Header {
            id: Header::check_component(id)?,
            ts,
            nonce: Header::check_component(nonce)?,
            mac,
            ext: ext
                .map(Into::into)
                .map(|value: String| {
                    quote_header(&value)?;
                    Ok::<String, Error>(value)
                })
                .transpose()?,
            hash,
            app: Header::check_component(app)?,
            dlg: Header::check_component(dlg)?,
        })
    }

    /// Check a header component for validity.
    fn check_component<S>(value: Option<S>) -> Result<Option<String>>
    where
        S: Into<String>,
    {
        if let Some(value) = value {
            let value = value.into();
            if value.contains('\"') {
                return Err(Error::HeaderParseError(
                    "Hawk headers cannot contain `\\`".into(),
                ));
            }
            Ok(Some(value))
        } else {
            Ok(None)
        }
    }

    /// Format the header for transmission in an Authorization header, omitting the `"Hawk "`
    /// prefix.
    pub fn fmt_header(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut sep = "";
        if let Some(ref id) = self.id {
            write!(f, "{sep}id={}", quote_header(id).map_err(|_| fmt::Error)?)?;
            sep = ", ";
        }
        if let Some(ref ts) = self.ts {
            write!(
                f,
                "{}ts=\"{}\"",
                sep,
                ts.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
            )?;
            sep = ", ";
        }
        if let Some(ref nonce) = self.nonce {
            write!(
                f,
                "{sep}nonce={}",
                quote_header(nonce).map_err(|_| fmt::Error)?
            )?;
            sep = ", ";
        }
        if let Some(ref mac) = self.mac {
            write!(f, "{}mac=\"{}\"", sep, b64::STANDARD_ENGINE.encode(mac))?;
            sep = ", ";
        }
        if let Some(ref ext) = self.ext {
            write!(f, "{sep}ext={}", quote_header(ext).map_err(|_| fmt::Error)?)?;
            sep = ", ";
        }
        if let Some(ref hash) = self.hash {
            write!(f, "{}hash=\"{}\"", sep, b64::STANDARD_ENGINE.encode(hash))?;
            sep = ", ";
        }
        if let Some(ref app) = self.app {
            write!(f, "{sep}app={}", quote_header(app).map_err(|_| fmt::Error)?)?;
            sep = ", ";
        }
        if let Some(ref dlg) = self.dlg {
            write!(f, "{sep}dlg={}", quote_header(dlg).map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl fmt::Display for Header {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.fmt_header(f)
    }
}

impl FromStr for Header {
    type Err = Error;
    fn from_str(s: &str) -> Result<Header> {
        let mut p = s;

        // Required attributes
        let mut id: Option<String> = None;
        let mut ts: Option<SystemTime> = None;
        let mut nonce: Option<String> = None;
        let mut mac: Option<Vec<u8>> = None;
        // Optional attributes
        let mut hash: Option<Vec<u8>> = None;
        let mut ext: Option<String> = None;
        let mut app: Option<String> = None;
        let mut dlg: Option<String> = None;

        while !p.is_empty() {
            // Skip whitespace and commas used as separators
            p = p.trim_start_matches(|c| c == ',' || char::is_whitespace(c));
            // Find first '=' which delimits attribute name from value
            let assign_end = p
                .find('=')
                .ok_or_else(|| Error::HeaderParseError("Expected '='".into()))?;
            let attr = &p[..assign_end].trim();
            if p.len() < assign_end + 1 {
                return Err(Error::HeaderParseError(
                    "Missing right hand side of =".into(),
                ));
            }
            p = p[assign_end + 1..].trim_start();
            if !p.starts_with('\"') {
                return Err(Error::HeaderParseError("Expected opening quote".into()));
            }
            let parsed = quoted_string::parse::<HttpQuoted>(p)
                .map_err(|_| Error::HeaderParseError("Invalid Hawk quoted field".into()))?;
            let val = quoted_string::to_content::<HttpQuoted>(parsed.quoted_string)
                .map_err(|_| Error::HeaderParseError("Invalid Hawk quoted field".into()))?
                .into_owned();
            let tail = parsed.tail;
            match *attr {
                "id" => id = Some(val.clone()),
                "ts" => {
                    let epoch = u64::from_str(&val)
                        .map_err(|_| Error::HeaderParseError("Error parsing `ts` field".into()))?;
                    ts = Some(UNIX_EPOCH.checked_add(Duration::new(epoch, 0)).ok_or_else(
                        || Error::HeaderParseError("Timestamp exceeds supported range".into()),
                    )?);
                }
                "mac" => {
                    mac = Some(b64::STANDARD_ENGINE.decode(&val).map_err(|_| {
                        Error::HeaderParseError("Error parsing `mac` field".into())
                    })?);
                }
                "nonce" => nonce = Some(val.clone()),
                "ext" => ext = Some(val.clone()),
                "hash" => {
                    hash = Some(b64::STANDARD_ENGINE.decode(&val).map_err(|_| {
                        Error::HeaderParseError("Error parsing `hash` field".into())
                    })?);
                }
                "app" => app = Some(val.clone()),
                "dlg" => dlg = Some(val.clone()),
                _ => {
                    return Err(Error::HeaderParseError(format!(
                        "Invalid Hawk field {}",
                        *attr
                    )))
                }
            };
            p = tail.trim_start();
            if !p.is_empty() && !p.starts_with(',') {
                return Err(Error::HeaderParseError(
                    "Expected Hawk attribute separator".into(),
                ));
            }
        }

        Ok(Header {
            id,
            ts,
            nonce,
            mac: mac.map(Mac::from),
            ext,
            hash,
            app,
            dlg,
        })
    }
}

#[cfg(test)]
mod test {
    use super::Header;
    use crate::mac::Mac;
    use std::str::FromStr;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn illegal_id() {
        assert!(Header::new(
            Some("ab\"cdef"),
            Some(UNIX_EPOCH + Duration::new(1234, 0)),
            Some("nonce"),
            Some(Mac::from(vec![])),
            Some("ext"),
            None,
            None,
            None
        )
        .is_err());
    }

    #[test]
    fn illegal_nonce() {
        assert!(Header::new(
            Some("abcdef"),
            Some(UNIX_EPOCH + Duration::new(1234, 0)),
            Some("no\"nce"),
            Some(Mac::from(vec![])),
            Some("ext"),
            None,
            None,
            None
        )
        .is_err());
    }

    #[test]
    fn escaped_ext_roundtrips() {
        let header = Header::new(
            Some("abcdef"),
            Some(UNIX_EPOCH + Duration::new(1234, 0)),
            Some("nonce"),
            Some(Mac::from(vec![])),
            Some("ex\"t"),
            None,
            None,
            None,
        )
        .unwrap();
        let encoded = header.to_string();
        let decoded = Header::from_str(&encoded).unwrap();
        assert_eq!(decoded.ext, header.ext);
    }

    #[test]
    fn illegal_app() {
        assert!(Header::new(
            Some("abcdef"),
            Some(UNIX_EPOCH + Duration::new(1234, 0)),
            Some("nonce"),
            Some(Mac::from(vec![])),
            None,
            None,
            Some("a\"pp"),
            None
        )
        .is_err());
    }

    #[test]
    fn illegal_dlg() {
        assert!(Header::new(
            Some("abcdef"),
            Some(UNIX_EPOCH + Duration::new(1234, 0)),
            Some("nonce"),
            Some(Mac::from(vec![])),
            None,
            None,
            None,
            Some("d\"lg")
        )
        .is_err());
    }

    #[test]
    fn from_str() {
        let s = Header::from_str(
            "id=\"dh37fgj492je\", ts=\"1353832234\", \
             nonce=\"j4h3g2\", ext=\"some-app-ext-data\", \
             mac=\"6R4rV5iE+NPoym+WwjeHzjAGXUtLNIxmo1vpMofpLAE=\", \
             hash=\"6R4rV5iE+NPoym+WwjeHzjAGXUtLNIxmo1vpMofpLAE=\", \
             app=\"my-app\", dlg=\"my-authority\"",
        )
        .unwrap();
        assert!(s.id == Some("dh37fgj492je".to_string()));
        assert!(s.ts == Some(UNIX_EPOCH + Duration::new(1353832234, 0)));
        assert!(s.nonce == Some("j4h3g2".to_string()));
        assert!(
            s.mac
                == Some(Mac::from(vec![
                    233, 30, 43, 87, 152, 132, 248, 211, 232, 202, 111, 150, 194, 55, 135, 206, 48,
                    6, 93, 75, 75, 52, 140, 102, 163, 91, 233, 50, 135, 233, 44, 1
                ]))
        );
        assert!(s.ext == Some("some-app-ext-data".to_string()));
        assert!(s.app == Some("my-app".to_string()));
        assert!(s.dlg == Some("my-authority".to_string()));
    }

    #[test]
    fn from_str_invalid_mac() {
        let r = Header::from_str(
            "id=\"dh37fgj492je\", ts=\"1353832234\", \
             nonce=\"j4h3g2\", ext=\"some-app-ext-data\", \
             mac=\"6!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!AE=\", \
             app=\"my-app\", dlg=\"my-authority\"",
        );
        assert!(r.is_err());
    }

    #[test]
    fn from_str_no_field() {
        let s = Header::from_str("").unwrap();
        assert!(s.id.is_none());
        assert!(s.ts.is_none());
        assert!(s.nonce.is_none());
        assert!(s.mac.is_none());
        assert!(s.ext.is_none());
        assert!(s.app.is_none());
        assert!(s.dlg.is_none());
    }

    #[test]
    fn from_str_few_field() {
        let s = Header::from_str(
            "id=\"xyz\", ts=\"1353832234\", \
             nonce=\"abc\", \
             mac=\"6R4rV5iE+NPoym+WwjeHzjAGXUtLNIxmo1vpMofpLAE=\"",
        )
        .unwrap();
        assert!(s.id == Some("xyz".to_string()));
        assert!(s.ts == Some(UNIX_EPOCH + Duration::new(1353832234, 0)));
        assert!(s.nonce == Some("abc".to_string()));
        assert!(
            s.mac
                == Some(Mac::from(vec![
                    233, 30, 43, 87, 152, 132, 248, 211, 232, 202, 111, 150, 194, 55, 135, 206, 48,
                    6, 93, 75, 75, 52, 140, 102, 163, 91, 233, 50, 135, 233, 44, 1
                ]))
        );
        assert!(s.ext.is_none());
        assert!(s.app.is_none());
        assert!(s.dlg.is_none());
    }

    #[test]
    fn from_str_messy() {
        let s = Header::from_str(
            ", id  =  \"dh37fgj492je\", ts=\"1353832234\", \
             nonce=\"j4h3g2\"  , , ext=\"some-app-ext-data\", \
             mac=\"6R4rV5iE+NPoym+WwjeHzjAGXUtLNIxmo1vpMofpLAE=\"",
        )
        .unwrap();
        assert!(s.id == Some("dh37fgj492je".to_string()));
        assert!(s.ts == Some(UNIX_EPOCH + Duration::new(1353832234, 0)));
        assert!(s.nonce == Some("j4h3g2".to_string()));
        assert!(
            s.mac
                == Some(Mac::from(vec![
                    233, 30, 43, 87, 152, 132, 248, 211, 232, 202, 111, 150, 194, 55, 135, 206, 48,
                    6, 93, 75, 75, 52, 140, 102, 163, 91, 233, 50, 135, 233, 44, 1
                ]))
        );
        assert!(s.ext == Some("some-app-ext-data".to_string()));
        assert!(s.app.is_none());
        assert!(s.dlg.is_none());
    }

    #[test]
    fn to_str_no_fields() {
        // must supply a type for S, since it is otherwise unused
        let s = Header::new::<String>(None, None, None, None, None, None, None, None).unwrap();
        let formatted = format!("{s}");
        println!("got: {formatted}");
        assert!(formatted.is_empty())
    }

    #[test]
    fn to_str_few_fields() {
        let s = Header::new(
            Some("dh37fgj492je"),
            Some(UNIX_EPOCH + Duration::new(1353832234, 0)),
            Some("j4h3g2"),
            Some(Mac::from(vec![
                8, 35, 182, 149, 42, 111, 33, 192, 19, 22, 94, 43, 118, 176, 65, 69, 86, 4, 156,
                184, 85, 107, 249, 242, 172, 200, 66, 209, 57, 63, 38, 83,
            ])),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        let formatted = format!("{s}");
        println!("got: {formatted}");
        assert!(
            formatted
                == "id=\"dh37fgj492je\", ts=\"1353832234\", nonce=\"j4h3g2\", \
                    mac=\"CCO2lSpvIcATFl4rdrBBRVYEnLhVa/nyrMhC0Tk/JlM=\""
        )
    }

    #[test]
    fn to_str_maximal() {
        let s = Header::new(
            Some("dh37fgj492je"),
            Some(UNIX_EPOCH + Duration::new(1353832234, 0)),
            Some("j4h3g2"),
            Some(Mac::from(vec![
                8, 35, 182, 149, 42, 111, 33, 192, 19, 22, 94, 43, 118, 176, 65, 69, 86, 4, 156,
                184, 85, 107, 249, 242, 172, 200, 66, 209, 57, 63, 38, 83,
            ])),
            Some("my-ext-value"),
            Some(vec![1, 2, 3, 4]),
            Some("my-app"),
            Some("my-dlg"),
        )
        .unwrap();
        let formatted = format!("{s}");
        println!("got: {formatted}");
        assert!(
            formatted
                == "id=\"dh37fgj492je\", ts=\"1353832234\", nonce=\"j4h3g2\", \
                    mac=\"CCO2lSpvIcATFl4rdrBBRVYEnLhVa/nyrMhC0Tk/JlM=\", ext=\"my-ext-value\", \
                    hash=\"AQIDBA==\", app=\"my-app\", dlg=\"my-dlg\""
        )
    }

    #[test]
    fn round_trip() {
        let s = Header::new(
            Some("dh37fgj492je"),
            Some(UNIX_EPOCH + Duration::new(1353832234, 0)),
            Some("j4h3g2"),
            Some(Mac::from(vec![
                8, 35, 182, 149, 42, 111, 33, 192, 19, 22, 94, 43, 118, 176, 65, 69, 86, 4, 156,
                184, 85, 107, 249, 242, 172, 200, 66, 209, 57, 63, 38, 83,
            ])),
            Some("my-ext-value"),
            Some(vec![1, 2, 3, 4]),
            Some("my-app"),
            Some("my-dlg"),
        )
        .unwrap();
        let formatted = format!("{s}");
        println!("got: {s}");
        let s2 = Header::from_str(&formatted).unwrap();
        assert!(s2 == s);
    }
}
