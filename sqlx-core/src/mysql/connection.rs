use std::{
    io,
    net::{IpAddr, SocketAddr},
};
use std::net::Shutdown;

use async_std::net::TcpStream;
use byteorder::{ByteOrder, LittleEndian};
use futures_util::AsyncWriteExt;

use crate::{Describe, Error, io::{Buf, BufMut, BufStream}, mysql::{
    protocol::{
        Capabilities, ColumnCountPacket, ColumnDefinitionPacket, ComPing, ComQuit,
        ComSetOption, ComStmtExecute,
        ComStmtPrepare, ComStmtPrepareOk, Encode, EofPacket, ErrPacket, OkPacket,
        ResultRow, SetOptionOptions, StmtExecFlag,
    },
    query::MySqlDbParameters,
}, Result, ResultField, url::Url};
use crate::mysql::MySql;
use crate::mysql::protocol::ComQuery;

use super::establish;

pub type StatementId = u32;

pub struct Connection {
    pub(crate) stream: BufStream<TcpStream>,
    pub(crate) rbuf: Vec<u8>,
    pub(crate) capabilities: Capabilities,
    next_seq_no: u8,
}

impl Connection {
    pub async fn open(url: Url) -> Result<Self> {
        // TODO: Handle errors
        let host = url.host();
        let port = url.port(3306);

        // TODO: handle errors
        let host: IpAddr = host.parse().unwrap();
        let addr: SocketAddr = (host, port).into();

        let stream = TcpStream::connect(&addr).await?;

        let mut conn = Self {
            stream: BufStream::new(stream),
            rbuf: Vec::with_capacity(8 * 1024),
            capabilities: Capabilities::empty(),
            next_seq_no: 0,
        };

        establish::establish(&mut conn, &url).await?;

        Ok(conn)
    }

    pub async fn close(mut self) -> Result<()> {
        // Send the quit command

        self.start_sequence();
        self.write(ComQuit);

        self.stream.flush().await?;
        self.stream.stream.shutdown(Shutdown::Both)?;

        Ok(())
    }

    pub async fn ping(&mut self) -> Result<()> {
        // Send the ping command and wait for (and drop) an OK packet

        self.start_sequence();
        self.write(ComPing);

        self.stream.flush().await?;

        let _ = self.receive_ok_or_err().await?;

        Ok(())
    }

    pub(crate) async fn receive(&mut self) -> Result<&[u8]> {
        Ok(self
            .try_receive()
            .await?
            .ok_or(Error::Io(io::ErrorKind::UnexpectedEof.into()))?)
    }

    async fn try_receive(&mut self) -> Result<Option<&[u8]>> {
        // Read the packet header which contains the length and the sequence number
        // https://mariadb.com/kb/en/library/0-packet/#standard-packet
        let mut header = ret_if_none!(self.stream.peek(4).await?);
        let len = header.get_u24::<LittleEndian>()? as usize;
        self.next_seq_no = header.get_u8()? + 1;
        self.stream.consume(4);

        // Read the packet body and copy it into our internal buf
        // We must have a separate buffer around the stream as we can't operate directly
        // on bytes returend from the stream. We have compression, split, etc. to
        // unpack.
        let body = ret_if_none!(self.stream.peek(len).await?);
        self.rbuf.clear();
        self.rbuf.extend_from_slice(body);
        self.stream.consume(len);

        Ok(Some(&self.rbuf[..len]))
    }

    pub(super) fn start_sequence(&mut self) {
        // At the start of a command sequence we reset our understanding
        // of [next_seq_no]. In a sequence our initial command must be 0, followed
        // by the server response that is 1, then our response to that response (if any),
        // would be 2
        self.next_seq_no = 0;
    }

    pub(crate) fn write<T: Encode>(&mut self, packet: T) {
        let buf = self.stream.buffer_mut();

        // Allocate room for the header that we write after the packet;
        // so, we can get an accurate and cheap measure of packet length

        let header_offset = buf.len();
        buf.advance(4);

        packet.encode(buf, self.capabilities);

        // Determine length of encoded packet
        // and write to allocated header

        let len = buf.len() - header_offset - 4;
        let mut header = &mut buf[header_offset..];

        LittleEndian::write_u32(&mut header, len as u32); // len

        // Take the last sequence number received, if any, and increment by 1
        // If there was no sequence number, we only increment if we split packets
        header[3] = self.next_seq_no;
        self.next_seq_no += 1;
    }

    // Decode an OK packet or bubble an ERR packet as an error
    // to terminate immediately
    pub(crate) async fn receive_ok_or_err(&mut self) -> Result<OkPacket> {
        let capabilities = self.capabilities;
        let buf = self.receive().await?;
        Ok(match buf[0] {
            0xfe | 0x00 => OkPacket::decode(buf, capabilities)?,

            0xff => {
                return ErrPacket::decode(buf)?.expect_error();
            }

            id => {
                return Err(protocol_err!(
                    "unexpected packet identifier 0x{:X?} when expecting 0xFE (OK) or 0xFF \
                     (ERR)",
                    id
                )
                .into());
            }
        })
    }

    async fn check_eof(&mut self) -> Result<()> {
        // When (legacy) EOFs are enabled, the fixed number column definitions are further
        // terminated by an EOF packet
        if !self
            .capabilities
            .contains(Capabilities::CLIENT_DEPRECATE_EOF)
        {
            let _eof = EofPacket::decode(self.receive().await?)?;
        }

        Ok(())
    }

    async fn send_prepare<'c>(
        &'c mut self,
        statement: &'c str,
    ) -> Result<ComStmtPrepareOk> {
        self.stream.flush().await?;

        self.start_sequence();
        self.write(ComStmtPrepare { statement });

        self.stream.flush().await?;

        // COM_STMT_PREPARE returns COM_STMT_PREPARE_OK (0x00) or ERR (0xFF)
        let packet = self.receive().await?;

        if packet[0] == 0xFF {
            return ErrPacket::decode(packet)?.expect_error();
        }

        let ok = ComStmtPrepareOk::decode(packet)?;

        Ok(ok)
    }

    // MySQL/Mysql responds with statement metadata for every PREPARE command
    // sometimes we care, sometimes we don't
    pub(super) async fn prepare_ignore_describe(&mut self, statement: &str) -> Result<StatementId> {
        let ok = self.send_prepare(statement).await?;

        if ok.params > 0 {
            // Input parameters
            for _ in 0..ok.params {
                // TODO: Maybe do something with this data ?
                let _column = ColumnDefinitionPacket::decode(self.receive().await?)?;
            }

            self.check_eof().await?;
        }

        if ok.columns > 0 {
            // Output parameters
            for _ in 0..ok.columns {
                // TODO: Maybe do something with this data ?
                let _column = ColumnDefinitionPacket::decode(self.receive().await?)?;
            }

            self.check_eof().await?;
        }

        Ok(ok.statement_id)
    }

    pub(super) async fn prepare_describe(&mut self, statement: &str) -> Result<Describe<MySql>> {
        let ok = self.send_prepare(statement).await?;

        let mut param_types = Vec::with_capacity(ok.params as usize);
        let mut result_fields= Vec::with_capacity(ok.columns as usize);

        // Input parameters
        for _ in 0..ok.params {
            let param = ColumnDefinitionPacket::decode(self.receive().await?)?;
            param_types.push(param.field_type.0);
        }

        self.check_eof().await?;

        // Output parameters
        for _ in 0..ok.columns {
            let column = ColumnDefinitionPacket::decode(self.receive().await?)?;
            result_fields.push(ResultField {
                name: column.column_alias.or(column.column),
                table_id: column.table_alias.or(column.table),
                type_id: column.field_type.0,
                _backcompat: ()
            });
        }

        self.check_eof().await?;

        Ok(Describe {
            param_types,
            result_fields,
            _backcompat: (),
        })
    }

    pub(super) async fn result_column_defs(&mut self) -> Result<Vec<ColumnDefinitionPacket>> {
        let packet = self.receive().await?;

        // A Resultset starts with a [ColumnCountPacket] which is a single field that encodes
        // how many columns we can expect when fetching rows from this statement

        if packet[0] == 255 {
            ErrPacket::decode(packet)?.expect_error()?;
        }

        let column_count: u64 = ColumnCountPacket::decode(packet)?.columns;

        // Next we have a [ColumnDefinitionPacket] which verbosely explains each minute
        // detail about the column in question including table, aliasing, and type
        // TODO: This information was *already* returned by PREPARE .., is there a way to suppress generation
        let mut columns = vec![];
        for _ in 0..column_count {
            let column = ColumnDefinitionPacket::decode(self.receive().await?)?;
            columns.push(column);
        }

        self.check_eof().await?;

        Ok(columns)
    }

    pub(super) async fn send_execute(
        &mut self,
        statement_id: u32,
        params: MySqlDbParameters,
    ) -> Result<()> {
        // TODO: EXECUTE(READ_ONLY) => FETCH instead of EXECUTE(NO)

        // SEND ================
        self.start_sequence();
        self.write(ComStmtExecute {
            statement_id,
            params: &params.params,
            null: &params.null_bitmap,
            flags: StmtExecFlag::NO_CURSOR,
            param_types: &params.param_types,
        });
        self.stream.flush().await?;
        // =====================

        Ok(())
    }

    async fn expect_eof_or_err(&mut self) -> crate::Result<()> {
        let packet = self.receive().await?;

        match packet[0] {
            0xFE => { EofPacket::decode(packet)?; },
            0xFF => { ErrPacket::decode(packet)?.expect_error()?; },
            _ => return Err(protocol_err!("expected EOF or ERR, got {:02X}", packet[0]).into()),
        }

        Ok(())
    }

    pub(super) async fn send_raw(
        &mut self,
        commands: &str
    ) -> Result<()> {
        self.stream.flush().await?;
        self.start_sequence();
        // enable multi-statement only for this query
        self.write(ComSetOption { option: SetOptionOptions::MySqlOptionMultiStatementsOn });
        self.write(ComQuery { sql_statement: commands });
        self.write(ComSetOption { option: SetOptionOptions::MySqlOptionMultiStatementsOff });
        self.stream.flush().await?;

        self.expect_eof_or_err().await?;

        let packet = self.receive().await?;

        if packet[0] == 0xFF { return ErrPacket::decode(packet)?.expect_error() }
        // otherwise ignore packet

        self.expect_eof_or_err().await?;

        Ok(())
    }
}
