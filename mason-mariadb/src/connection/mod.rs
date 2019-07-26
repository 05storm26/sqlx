use crate::protocol::{
    deserialize::{DeContext, Deserialize},
    encode::Encoder,
    packets::{com_ping::ComPing, com_query::ComQuery, com_quit::ComQuit, com_init_db::ComInitDb, ok::OkPacket},
    serialize::Serialize,
    server::Message as ServerMessage,
    types::{Capabilities, ServerStatusFlag},
};
use byteorder::{ByteOrder, LittleEndian};
use bytes::{Bytes, BytesMut};
use failure::Error;
use futures::{
    io::{AsyncRead, AsyncWriteExt},
    prelude::*,
};
use mason_core::ConnectOptions;
use runtime::net::TcpStream;

mod establish;

pub struct Connection {
    pub stream: Framed,

    // Buffer used when serializing outgoing messages
    pub encoder: Encoder,

    // Context for the connection
    // Explicitly declared to easily send to deserializers
    pub context: ConnContext,
}

pub struct ConnContext {
    // MariaDB Connection ID
    pub connection_id: i32,

    // Sequence Number
    pub seq_no: u8,

    // Last sequence number return by MariaDB
    pub last_seq_no: u8,

    // Server Capabilities
    pub capabilities: Capabilities,

    // Server status
    pub status: ServerStatusFlag,
}

impl Connection {
    pub async fn establish(options: ConnectOptions<'static>) -> Result<Self, Error> {
        let stream: Framed = Framed::new(TcpStream::connect((options.host, options.port)).await?);
        let mut conn: Connection = Self {
            stream,
            encoder: Encoder::new(1024),
            context: ConnContext {
                connection_id: -1,
                seq_no: 1,
                last_seq_no: 0,
                capabilities: Capabilities::default(),
                status: ServerStatusFlag::default(),
            },
        };

        establish::establish(&mut conn, options).await?;

        Ok(conn)
    }

    pub async fn send<S>(&mut self, message: S) -> Result<(), Error>
    where
        S: Serialize,
    {
        self.encoder.clear();
        self.encoder.alloc_packet_header();
        self.encoder.seq_no(self.context.seq_no);
        message.serialize(self)?;
        self.encoder.encode_length();

        self.stream.inner.write_all(&self.encoder.buf).await?;
        self.stream.inner.flush().await?;

        Ok(())
    }

    pub async fn quit(&mut self) -> Result<(), Error> {
        self.context.seq_no = 0;
        self.send(ComQuit()).await?;

        Ok(())
    }

    pub async fn query<'a>(&'a mut self, sql_statement: &'a str) -> Result<(), Error> {
        self.context.seq_no = 0;
        self.send(ComQuery { sql_statement: bytes::Bytes::from(sql_statement) }).await?;

        Ok(())
    }


    pub async fn select_db<'a>(&'a mut self, db: &'a str) -> Result<(), Error> {
        self.context.seq_no = 0;
        self.send(ComInitDb { schema_name: bytes::Bytes::from(db) }).await?;

        Ok(())
    }

    pub async fn ping(&mut self) -> Result<(), Error> {
        self.context.seq_no = 0;
        self.send(ComPing()).await?;

        // Ping response must be an OkPacket
        let buf = self.stream.next_bytes().await?;
        OkPacket::deserialize(&mut DeContext::new(&mut self.context, &buf))?;

        Ok(())
    }

    pub async fn next(&mut self) -> Result<Option<ServerMessage>, Error> {
        let mut rbuf = BytesMut::new();
        let mut len = 0;

        loop {
            if len == rbuf.len() {
                rbuf.reserve(32);

                unsafe {
                    // Set length to the capacity and efficiently
                    // zero-out the memory
                    rbuf.set_len(rbuf.capacity());
                    self.stream.inner.initializer().initialize(&mut rbuf[len..]);
                }
            }

            let bytes_read = self.stream.inner.read(&mut rbuf[len..]).await?;

            if bytes_read > 0 {
                len += bytes_read;
            } else {
                // Read 0 bytes from the server; end-of-stream
                break;
            }

            while len > 0 {
                let size = rbuf.len();
                let message = ServerMessage::deserialize(&mut DeContext::new(
                    &mut self.context,
                    &rbuf.as_ref().into(),
                ))?;
                len -= size - rbuf.len();

                match message {
                    message @ Some(_) => return Ok(message),
                    // Did not receive enough bytes to
                    // deserialize a complete message
                    None => break,
                }
            }
        }

        Ok(None)
    }
}

pub struct Framed {
    inner: TcpStream,
    readable: bool,
    eof: bool,
    buffer: BytesMut,
}

impl Framed {
    fn new(stream: TcpStream) -> Self {
        Self {
            readable: false,
            eof: false,
            inner: stream,
            buffer: BytesMut::with_capacity(8 * 1024),
        }
    }

    pub async fn next_bytes(&mut self) -> Result<Bytes, Error> {
        let mut rbuf = BytesMut::new();
        let mut len = 0;
        let mut packet_len: u32 = 0;

        loop {
            if len == rbuf.len() {
                rbuf.reserve(20000);

                unsafe {
                    // Set length to the capacity and efficiently
                    // zero-out the memory
                    rbuf.set_len(rbuf.capacity());
                    self.inner.initializer().initialize(&mut rbuf[len..]);
                }
            }

            let bytes_read = self.inner.read(&mut rbuf[len..]).await?;

            if bytes_read > 0 {
                len += bytes_read;
            } else {
                // Read 0 bytes from the server; end-of-stream
                return Ok(Bytes::new());
            }

            if len > 0 && packet_len == 0 {
                packet_len = LittleEndian::read_u24(&rbuf[0..]);
            }

            // Loop until the length of the buffer is the length of the packet
            if packet_len as usize > len {
                continue;
            } else {
                return Ok(rbuf.freeze());
            }
        }
    }
}
