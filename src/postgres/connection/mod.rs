use super::protocol::{Encode, Message, Terminate};
use bytes::{BufMut, BytesMut};
use futures::{
    future::BoxFuture,
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
    ready,
    task::{Context, Poll},
    Future,
};
use runtime::net::TcpStream;
use std::{fmt::Debug, io, pin::Pin};
use url::Url;

mod establish;
mod execute;
mod get;
mod prepare;
mod select;

pub struct RawConnection {
    stream: TcpStream,

    // Do we think that there is data in the read buffer to be decoded
    stream_readable: bool,

    // Have we reached end-of-file (been disconnected)
    stream_eof: bool,

    // Buffer used when sending outgoing messages
    wbuf: Vec<u8>,

    // Buffer used when reading incoming messages
    // TODO: Evaluate if we _really_ want to use BytesMut here
    rbuf: BytesMut,

    // Process ID of the Backend
    process_id: u32,

    // Backend-unique key to use to send a cancel query message to the server
    secret_key: u32,
}

impl RawConnection {
    pub async fn establish(url: &Url) -> io::Result<Self> {
        let host = url.host_str().unwrap_or("localhost");
        let port = url.port().unwrap_or(5432);

        let stream = TcpStream::connect((host, port)).await?;
        let mut conn = Self {
            wbuf: Vec::with_capacity(1024),
            rbuf: BytesMut::with_capacity(1024 * 8),
            stream,
            stream_readable: false,
            stream_eof: false,
            process_id: 0,
            secret_key: 0,
        };

        establish::establish(&mut conn, &url).await?;

        Ok(conn)
    }

    pub fn prepare<'a, 'b>(&'a mut self, query: &'b str) -> prepare::Prepare<'a, 'b> {
        prepare::prepare(self, query)
    }

    pub async fn close(mut self) -> io::Result<()> {
        self.write(Terminate);
        self.flush().await?;
        self.stream.close().await?;

        Ok(())
    }

    // Wait and return the next message to be received from Postgres.
    async fn receive(&mut self) -> io::Result<Option<Message>> {
        loop {
            if self.stream_eof {
                // Reached end-of-file on a previous read call.
                return Ok(None);
            }

            if self.stream_readable {
                loop {
                    match Message::decode(&mut self.rbuf)? {
                        Some(Message::ParameterStatus(_body)) => {
                            // TODO: not sure what to do with these yet
                        }

                        Some(Message::Response(_body)) => {
                            // TODO: Transform Errors+ into an error type and return
                            // TODO: Log all others
                        }

                        Some(message) => {
                            return Ok(Some(message));
                        }

                        None => {
                            // Not enough data in the read buffer to parse a message
                            self.stream_readable = true;
                            break;
                        }
                    }
                }
            }

            // Ensure there is at least 32-bytes of space available
            // in the read buffer so we can safely detect end-of-file
            self.rbuf.reserve(32);

            // SAFE: Read data in directly to buffer without zero-initializing the data.
            //       Postgres is a self-describing format and the TCP frames encode
            //       length headers. We will never attempt to decode more than we
            //       received.
            let n = self.stream.read(unsafe { self.rbuf.bytes_mut() }).await?;

            // SAFE: After we read in N bytes, we can tell the buffer that it actually
            //       has that many bytes MORE for the decode routines to look at
            unsafe { self.rbuf.advance_mut(n) }

            if n == 0 {
                self.stream_eof = true;
            }

            self.stream_readable = true;
        }
    }

    fn write(&mut self, message: impl Encode) {
        message.encode(&mut self.wbuf);
    }

    async fn flush(&mut self) -> io::Result<()> {
        self.stream.write_all(&self.wbuf).await?;
        self.wbuf.clear();

        Ok(())
    }
}

impl crate::connection::RawConnection for RawConnection {
    #[inline]
    fn establish(url: &Url) -> BoxFuture<io::Result<Self>> {
        Box::pin(RawConnection::establish(url))
    }
}
