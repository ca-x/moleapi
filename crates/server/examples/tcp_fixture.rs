//! Local manual QA echo fixture using the same mature wire codecs, never production listener.
use anyhow::Result;
use bytes::{Bytes, BytesMut};
use futures_util::{SinkExt, StreamExt};
use tokio_util::codec::{BytesCodec, Decoder, Encoder, Framed, LengthDelimitedCodec, LinesCodec};
async fn echo<C, O>(
    mut stream: Framed<tokio::net::TcpStream, C>,
    encode: fn(C::Item) -> O,
    eof: fn() -> O,
) -> Result<()>
where
    C: Decoder + Encoder<O> + Unpin,
    C::Item: Send,
    O: Send,
    <C as Decoder>::Error: std::error::Error + Send + Sync + 'static,
    <C as Encoder<O>>::Error: std::error::Error + Send + Sync + 'static,
{
    while let Some(message) = stream.next().await {
        stream.send(encode(message?)).await?;
    }
    stream.send(eof()).await?;
    stream.close().await?;
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let port: u16 = args.next().unwrap_or("19011".into()).parse()?;
    let framing = args.next().unwrap_or("raw".into());
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    println!("Local TCP {framing} fixture: tcp://127.0.0.1:{port}");
    loop {
        let (stream, _) = listener.accept().await?;
        let framing = framing.clone();
        tokio::spawn(async move {
            let result = match framing.as_str() {
                "lines" => {
                    echo(
                        Framed::new(stream, LinesCodec::new_with_max_length(1048576)),
                        |s: String| s,
                        || "peer EOF acknowledged".to_owned(),
                    )
                    .await
                }
                "length_be" | "length_le" => {
                    let mut codec = LengthDelimitedCodec::builder();
                    codec.length_field_length(4).max_frame_length(1048576);
                    if framing == "length_le" {
                        codec.little_endian();
                    }
                    echo(
                        Framed::new(stream, codec.new_codec()),
                        |s: BytesMut| s.freeze(),
                        || Bytes::from_static(b"peer EOF acknowledged"),
                    )
                    .await
                }
                _ => {
                    echo(
                        Framed::new(stream, BytesCodec::new()),
                        |s: BytesMut| s.freeze(),
                        || Bytes::from_static(b"peer EOF acknowledged"),
                    )
                    .await
                }
            };
            if result.is_err() {
                eprintln!("Local TCP fixture peer stopped");
            }
        });
    }
}
