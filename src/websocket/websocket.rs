fn compute_accept_key(req: &Request<hyper::body::Incoming>) -> String {
    let key = req
        .headers()
        .get("Sec-WebSocket-Key")
        .expect("Missing Sec-WebSocket-Key");
    let mut hasher = Sha1::new();
    hasher.update(key.as_bytes());
    hasher.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
    general_purpose::STANDARD.encode(hasher.finalize())
}

pub async fn serve() {
    let mut buffer = [0u8; 1024];
    match socket.read(&mut buffer).await {
        Ok(n) if n == 0 => return,
        Ok(n) => {
            let req = String::from_utf8_lossy(&buffer[..n]);
            if req.starts_with("GET /ws") {
                if let Some(key_line) = req
                    .lines()
                    .find(|line| line.starts_with("Sec-WebSocket-Key:"))
                {
                    let key = key_line.trim().split(": ").nth(1).unwrap_or("");
                    let mut hasher = Sha1::new();
                    hasher.update(key.as_bytes());
                    hasher.update(b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11");
                    let accept = general_purpose::STANDARD.encode(hasher.finalize());

                    let response = format!(
                        "HTTP/1.1 101 Switching Protocols\r\n\
                        Upgrade: websocket\r\n\
                        Connection: Upgrade\r\n\
                        Sec-WebSocket-Accept: {}\r\n\r\n",
                        accept
                    );

                    if let Err(e) = socket.write_all(response.as_bytes()).await {
                        eprintln!("Handshake write error: {:?}", e);
                        return;
                    }

                    _ = handle_websocket_stream(socket).await;
                }
            }
        }
    }
}

use core::time::Duration;
use std::io;
use std::pin::Pin;



// dynamic traits seem to need heap allocation; can we use the connection's small heap for this?
pub trait AsyncReadWrite: AsyncRead + AsyncWrite + Send + Unpin {}
pub type MessageStream = WsFramed<Box<dyn AsyncReadWrite>>;
impl AsyncReadWrite for tokio::net::TcpStream {}
impl<T> AsyncReadWrite for Pin<&mut T> where T: AsyncReadWrite {}

pub struct DynWsFramed {
    inner: WsFramed<Box<dyn AsyncReadWrite + Send + Unpin>>,
}
impl DynWsFramed {
    pub fn new(stream: Box<dyn AsyncReadWrite + Unpin + Send>) -> Self {
        Self {
            inner: WsFramed::new(stream),
        }
    }
}
use std::ops::{Deref, DerefMut};

impl Deref for DynWsFramed {
    type Target = WsFramed<Box<dyn AsyncReadWrite + Unpin + Send>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for DynWsFramed {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub struct WsFramed<R: AsyncRead + AsyncWrite + Unpin + Send> {
    stream: R,
    frame_remaining: u64,
    message_done: bool,
    masking_key: Option<[u8; 4]>,
    opcode: u8,
}

impl<R: AsyncRead + AsyncWrite + Unpin + Send> WsFramed<R> {
    pub fn new(reader: R) -> Self {
        Self {
            stream: reader,
            frame_remaining: 0,
            message_done: true, // force frame read at start
            masking_key: None,
            opcode: 0,
        }
    }

    pub async fn write_message(&mut self, buf: &[u8], opcode: u8) -> io::Result<()> {
        let fin_opcode = 0x80 | (opcode & 0x0F);
        let (len_byte, ext) = match buf.len() {
            len if len < 126 => (len as u8, vec![]),
            len if len <= u16::MAX as usize => {
                let mut ext = vec![0u8; 2];
                ext[..].copy_from_slice(&(len as u16).to_be_bytes());
                (126, ext)
            }
            len => {
                let mut ext = vec![0u8; 8];
                ext[..].copy_from_slice(&(len as u64).to_be_bytes());
                (127, ext)
            }
        };

        let mut header = vec![fin_opcode, len_byte];
        header.extend_from_slice(&ext);
        self.stream.write_all(&header).await?;
        self.stream.write_all(buf).await?;
        Ok(())
    }

    pub async fn write_message_chunk(&mut self, buf: &[u8], opcode: u8) -> io::Result<()> {
        let fin_opcode = opcode & 0x0F; // FIN bit = 0
        let (len_byte, ext) = match buf.len() {
            len if len < 126 => (len as u8, vec![]),
            len if len <= u16::MAX as usize => {
                let mut ext = vec![0u8; 2];
                ext[..].copy_from_slice(&(len as u16).to_be_bytes());
                (126, ext)
            }
            len => {
                let mut ext = vec![0u8; 8];
                ext[..].copy_from_slice(&(len as u64).to_be_bytes());
                (127, ext)
            }
        };

        let mut header = vec![fin_opcode, len_byte];
        header.extend_from_slice(&ext);
        self.stream.write_all(&header).await?;
        self.stream.write_all(buf).await?;
        Ok(())
    }

    pub async fn write_message_finish(&mut self, buf: &[u8]) -> io::Result<()> {
        let fin_opcode = 0x80; // FIN bit set, opcode = 0 (continuation)
        let (len_byte, ext) = match buf.len() {
            len if len < 126 => (len as u8, vec![]),
            len if len <= u16::MAX as usize => {
                let mut ext = vec![0u8; 2];
                ext[..].copy_from_slice(&(len as u16).to_be_bytes());
                (126, ext)
            }
            len => {
                let mut ext = vec![0u8; 8];
                ext[..].copy_from_slice(&(len as u64).to_be_bytes());
                (127, ext)
            }
        };

        let mut header = vec![fin_opcode, len_byte];
        header.extend_from_slice(&ext);
        self.stream.write_all(&header).await?;
        self.stream.write_all(buf).await?;
        Ok(())
    }

    pub async fn read_some_timeout(
        &mut self,
        buf: &mut [u8],
        duration: Duration,
    ) -> io::Result<(usize, bool)> {
        match timeout(duration, self.read_some(buf)).await {
            Ok(res) => res,
            Err(_) => Err(io::Error::new(io::ErrorKind::TimedOut, "read timed out")),
        }
    }
    pub async fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream.read_exact(buf).await
    }
    pub async fn read_some(&mut self, buf: &mut [u8]) -> io::Result<(usize, bool)> {
        // If current frame is done, read a new frame header
        while self.frame_remaining == 0 {
            let mut header = [0u8; 2];
            self.stream.read_exact(&mut header).await?;

            let fin = header[0] & 0x80 != 0;
            self.opcode = header[0] & 0x0F;
            let masked = header[1] & 0x80 != 0;
            let mut payload_len = (header[1] & 0x7F) as u64;

            if payload_len == 126 {
                let mut ext = [0u8; 2];
                self.stream.read_exact(&mut ext).await?;
                payload_len = u16::from_be_bytes(ext) as u64;
            } else if payload_len == 127 {
                let mut ext = [0u8; 8];
                self.stream.read_exact(&mut ext).await?;
                payload_len = u64::from_be_bytes(ext);
            }

            self.masking_key = if masked {
                let mut key = [0u8; 4];
                self.stream.read_exact(&mut key).await?;
                Some(key)
            } else {
                None
            };

            if self.opcode == 0x8 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "WebSocket close frame",
                ));
            } else if self.opcode != 0x0 && self.opcode != 0x1 && self.opcode != 0x2 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "unsupported opcode",
                ));
            }

            self.frame_remaining = payload_len;
            self.message_done = fin;
        }

        // Read as much of this frame as possible into buf
        let to_read = std::cmp::min(buf.len() as u64, self.frame_remaining) as usize;
        self.stream.read_exact(&mut buf[..to_read]).await?;

        if let Some(key) = self.masking_key {
            let offset = self.frame_remaining as usize - to_read;
            for i in 0..to_read {
                buf[i] ^= key[(offset + i) % 4];
            }
        }

        self.frame_remaining -= to_read as u64;

        let message_ended = self.frame_remaining == 0 && self.message_done;
        Ok((to_read, message_ended))
    }

    // this will return even if the message is not complete.
    // we would know the length of the message though, and the rest has to be sequential on the connection.
    pub async fn read_message_timeout(
        &mut self,
        buf: &mut [u8],
        duration: Duration,
    ) -> io::Result<usize> {
        let mut total_read = 0;
        loop {
            let (n, message_done) = match timeout(duration, self.read_some(&mut buf[total_read..]))
                .await
            {
                Ok(Ok(res)) => res,
                Ok(Err(e)) => return Err(e),
                Err(_) => return Err(io::Error::new(io::ErrorKind::TimedOut, "read timed out")),
            };
            total_read += n;
            if message_done || total_read == buf.len() {
                return Ok(total_read);
            }
        }
    }
}

/*
    loop {
        let mut header = [0u8; 2];
        ws.read_exact(&mut header).await?;

        let fin = header[0] & 0x80 != 0;
        let opcode = header[0] & 0x0F;
        let masked = header[1] & 0x80 != 0;

        if opcode == 0x8 {
            println!("Received close frame. Sending Close frame back and closing connection.");
            // Send Close frame in response
            let close_response = [0x88, 0x00]; // FIN=1, opcode=0x8 (Close), length=0
            let _ = stream.write_all(&close_response).await;
            break Ok(());
        }

        let mut payload_len = (header[1] & 0x7F) as u64;

        if payload_len == 126 {
            let mut extended = [0u8; 2];
            ws.read_exact(&mut extended).await?;
            payload_len = u16::from_be_bytes(extended) as u64;
        } else if payload_len == 127 {
            let mut extended = [0u8; 8];
            ws.read_exact(&mut extended).await?;
            payload_len = u64::from_be_bytes(extended);
        }

        let masking_key = if masked {
            let mut key = [0u8; 4];
            ws.read_exact(&mut key).await?;
            Some(key)
        } else {
            None
        };

        let mut remaining = payload_len;
        while remaining > 0 {
            let chunk_size = std::cmp::min(remaining, MAX_CHUNK as u64) as usize;
            let mut buf = vec![0u8; chunk_size];
            ws.read_exact(&mut buf).await?;
            remaining -= chunk_size as u64;

            if let Some(key) = masking_key {
                for i in 0..buf.len() {
                    buf[i] ^= key[i % 4];
                }
            }
            let msg = String::from_utf8_lossy(&buf);
            println!(
                "Frame (opcode={opcode}, fin={fin}) — chunk: {} bytes {}",
                buf.len(),
                msg
            );
        }
    }
}*/
