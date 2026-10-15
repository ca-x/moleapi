//! Isolated wire probe for a candidate dependency. Not product transport.
use anyhow::{Context, Result, ensure};
use dubbo_rs_common::url::URL;
use dubbo_rs_protocol::InvocationContext;
use dubbo_rs_protocol_dubbo::{DubboCodec, body, codec::SerializationId};
use dubbo_rs_remoting::{Codec, Request};
use std::time::Duration;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[tokio::main]
async fn main() -> Result<()> {
    let port: u16 = std::env::args().nth(1).context("port argument")?.parse()?;
    let mut url = URL::new("dubbo", "/fixture.EchoService");
    url.ip = "127.0.0.1".into();
    url.port = port.to_string();
    let context = InvocationContext::new("echo", url)
        .with_parameter_types(vec!["Ljava/lang/String;".into()])
        .with_arguments(vec![b"moleapi".to_vec()]);
    let body = body::encode_request_body(&context)?;
    let codec = DubboCodec::new(SerializationId::Hessian2);
    let request = Request {
        id: 91,
        is_twoway: true,
        is_event: false,
        data: body.clone(),
    };
    let frame = codec.encode_request(&request)?;
    ensure!(frame.len() < 65_536, "Candidate probe frame too large");
    println!("candidate_body_hex={}", hex::encode(&body));
    let run = async {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).await?;
        stream.write_all(&frame).await?;
        let mut header = [0u8; 16];
        stream.read_exact(&mut header).await?;
        let size = u32::from_be_bytes(header[12..16].try_into()?) as usize;
        ensure!(size <= 65_536, "Peer response exceeds probe limit");
        let mut response = header.to_vec();
        response.resize(16 + size, 0);
        stream.read_exact(&mut response[16..]).await?;
        let decoded = codec.decode_response(&response)?;
        println!("response_status={}", decoded.status);
        println!("response_body_hex={}", hex::encode(&decoded.data));
        if decoded.is_error() {
            println!("response_error={}", String::from_utf8_lossy(&decoded.data));
        } else {
            match body::decode_response_body(&decoded.data) {
                Ok(result) => {
                    println!("candidate_decoded={result:?}");
                    if let Some(value) = result.value {
                        println!("candidate_value_utf8={}", String::from_utf8_lossy(&value));
                    }
                }
                Err(error) => println!("candidate_decode_error={error}"),
            }
        }
        Ok::<(), anyhow::Error>(())
    };
    tokio::time::timeout(Duration::from_secs(15), run)
        .await
        .context("Probe timed out")??;
    Ok(())
}
