use super::super::{
    deserialize::{DeContext, Deserialize},
    types::ServerStatusFlag,
};
use bytes::Bytes;
use failure::{err_msg, Error};

#[derive(Default, Debug)]
pub struct OkPacket {
    pub length: u32,
    pub seq_no: u8,
    pub affected_rows: Option<usize>,
    pub last_insert_id: Option<usize>,
    pub server_status: ServerStatusFlag,
    pub warning_count: u16,
    pub info: Bytes,
    pub session_state_info: Option<Bytes>,
    pub value: Option<Bytes>,
}

impl Deserialize for OkPacket {
    fn deserialize(ctx: &mut DeContext) -> Result<Self, Error> {
        let decoder = &mut ctx.decoder;
        // Packet header
        let length = decoder.decode_length()?;
        let seq_no = decoder.decode_int_1();

        // Packet body
        let packet_header = decoder.decode_int_1();
        if packet_header != 0 && packet_header != 0xFE {
            return Err(err_msg("Packet header is not 0 or 0xFE for OkPacket"));
        }

        let affected_rows = decoder.decode_int_lenenc();
        let last_insert_id = decoder.decode_int_lenenc();
        let server_status = ServerStatusFlag::from_bits_truncate(decoder.decode_int_2().into());
        let warning_count = decoder.decode_int_2();

        // Assuming CLIENT_SESSION_TRACK is unsupported
        let session_state_info = None;
        let value = None;

        let info = decoder.decode_byte_eof();

        Ok(OkPacket {
            length,
            seq_no,
            affected_rows,
            last_insert_id,
            server_status,
            warning_count,
            info,
            session_state_info,
            value,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{__bytes_builder, connection::Connection, protocol::decode::Decoder};
    use mason_core::ConnectOptions;

    #[runtime::test]
    async fn it_decodes_ok_packet() -> Result<(), Error> {
        let mut conn = Connection::establish(ConnectOptions {
            host: "127.0.0.1",
            port: 3306,
            user: Some("root"),
            database: None,
            password: None,
        })
        .await?;

        #[rustfmt::skip]
        let buf = __bytes_builder!(
            // length
            0x0F_u8, 0x0_u8, 0x0_u8,
            // seq_no
            0x01_u8,
            // 0x00 : OK_Packet header or (0xFE if CLIENT_DEPRECATE_EOF is set)
            0x00_u8,
            // int<lenenc> affected rows
            0xFB_u8,
            // int<lenenc> last insert id
            0xFB_u8,
            // int<2> server status
            0x01_u8, 0x01_u8,
            // int<2> warning count
            0x0_u8, 0x0_u8,
            // if session_tracking_supported (see CLIENT_SESSION_TRACK) {
            //   string<lenenc> info
            //   if (status flags & SERVER_SESSION_STATE_CHANGED) {
            //     string<lenenc> session state info
            //     string<lenenc> value of variable
            //   }
            // } else {
            //   string<EOF> info
                b"info"
            // }
        );

        let message = OkPacket::deserialize(&mut DeContext::new(&mut conn.context, &buf))?;

        assert_eq!(message.affected_rows, None);
        assert_eq!(message.last_insert_id, None);
        assert!(!(message.server_status & ServerStatusFlag::SERVER_STATUS_IN_TRANS).is_empty());
        assert_eq!(message.warning_count, 0);
        assert_eq!(message.info, b"info".to_vec());

        Ok(())
    }
}
