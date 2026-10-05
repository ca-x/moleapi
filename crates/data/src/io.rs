use std::{
    io,
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Poll},
};
use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    net::TcpStream,
};
pub const MAX_WIRE: usize = 20 * 1024 * 1024;
pub struct BudgetSocket {
    inner: TcpStream,
    bytes: Arc<AtomicUsize>,
}
impl BudgetSocket {
    pub fn new(inner: TcpStream) -> Self {
        Self {
            inner,
            bytes: Arc::new(AtomicUsize::new(0)),
        }
    }
}
impl AsyncRead for BudgetSocket {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.bytes.load(Ordering::Relaxed) >= MAX_WIRE {
            return Poll::Ready(Err(io::Error::other(
                "Data connection wire budget exceeded",
            )));
        }
        let available = MAX_WIRE - self.bytes.load(Ordering::Relaxed);
        let capacity = buf.remaining().min(available);
        let mut limited = ReadBuf::new(buf.initialize_unfilled_to(capacity));
        let result = Pin::new(&mut self.inner).poll_read(cx, &mut limited);
        if matches!(result, Poll::Ready(Ok(()))) {
            let n = limited.filled().len();
            buf.advance(n);
            self.bytes.fetch_add(n, Ordering::Relaxed);
        }
        result
    }
}
impl AsyncWrite for BudgetSocket {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let used = self.bytes.load(Ordering::Relaxed);
        if used >= MAX_WIRE {
            return Poll::Ready(Err(io::Error::other(
                "Data connection wire budget exceeded",
            )));
        }
        let length = buf.len().min(MAX_WIRE - used);
        let result = Pin::new(&mut self.inner).poll_write(cx, &buf[..length]);
        if let Poll::Ready(Ok(n)) = result {
            self.bytes.fetch_add(n, Ordering::Relaxed);
        }
        result
    }
    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }
    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}
