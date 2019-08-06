use byteorder::{ByteOrder, LittleEndian};
use bytes::{BufMut, Bytes, BytesMut};
use crate::mariadb::FieldType;

pub const U24_MAX: usize = 0xFF_FF_FF;

// A simple wrapper around a BytesMut to easily encode values
pub struct Encoder {
    pub buf: BytesMut,
}

impl Encoder {
    // Create a new Encoder with a given capacity
    pub fn new(capacity: usize) -> Self {
        Encoder { buf: BytesMut::with_capacity(capacity) }
    }

    // Clears the encoding buffer
    pub fn clear(&mut self) {
        self.buf.clear();
    }

    // Reserve space for packet header; Packet Body Length (3 bytes) and sequence number (1 byte)
    #[inline]
    pub fn alloc_packet_header(&mut self) {
        self.buf.extend_from_slice(&[0; 4]);
    }

    // Encode the sequence number; the 4th byte of the packet
    #[inline]
    pub fn seq_no(&mut self, seq_no: u8) {
        self.buf[3] = seq_no;
    }

    // Encode the sequence number; the first 3 bytes of the packet in little endian format
    #[inline]
    pub fn encode_length(&mut self) {
        let mut length = [0; 3];
        if self.buf.len() > U24_MAX {
            panic!("Buffer too long");
        } else if self.buf.len() < 4 {
            panic!("Buffer too short. Only contains packet length and sequence number")
        }

        LittleEndian::write_u24(&mut length, self.buf.len() as u32 - 4);

        // Set length at the start of the buffer
        // sadly there is no `prepend` for rust Vec
        self.buf[0] = length[0];
        self.buf[1] = length[1];
        self.buf[2] = length[2];
    }

    // Encode a u64 as an int<8>
    #[inline]
    pub fn encode_int_u64(&mut self, value: u64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    // Encode a i64 as an int<8>
    #[inline]
    pub fn encode_int_i64(&mut self, value: i64) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    #[inline]
    pub fn encode_int_8(&mut self, bytes: &Bytes) {
        self.buf.extend_from_slice(bytes);
    }

    // Encode a u32 as an int<4>
    #[inline]
    pub fn encode_int_u32(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    // Encode a i32 as an int<4>
    #[inline]
    pub fn encode_int_i32(&mut self, value: i32) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    #[inline]
    pub fn encode_int_4(&mut self, bytes: &Bytes) {
        self.buf.extend_from_slice(bytes);
    }

    // Encode a u32 (truncated to u24) as an int<3>
    #[inline]
    pub fn encode_int_u24(&mut self, value: u32) {
        self.buf.extend_from_slice(&value.to_le_bytes()[0..3]);
    }
    // Encode a i32 (truncated to i24) as an int<3>
    #[inline]
    pub fn encode_int_i24(&mut self, value: i32) {
        self.buf.extend_from_slice(&value.to_le_bytes()[0..3]);
    }

    // Encode a u16 as an int<2>
    #[inline]
    pub fn encode_int_u16(&mut self, value: u16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    // Encode a i16 as an int<2>
    #[inline]
    pub fn encode_int_i16(&mut self, value: i16) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    #[inline]
    pub fn encode_int_2(&mut self, bytes: &Bytes) {
        self.buf.extend_from_slice(bytes);
    }

    // Encode a u8 as an int<1>
    #[inline]
    pub fn encode_int_u8(&mut self, value: u8) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    #[inline]
    pub fn encode_int_1(&mut self, bytes: &Bytes) {
        self.buf.extend_from_slice(bytes);
    }

    // Encode a i8 as an int<1>
    #[inline]
    pub fn encode_int_i8(&mut self, value: i8) {
        self.buf.extend_from_slice(&value.to_le_bytes());
    }

    // Encode an int<lenenc>; length encoded int
    // See Decoder::decode_int_lenenc for explanation of how int<lenenc> is encoded
    #[inline]
    pub fn encode_int_lenenc(&mut self, value: Option<&usize>) {
        if let Some(value) = value {
            if *value > U24_MAX && *value <= std::u64::MAX as usize {
                self.buf.put_u8(0xFE);
                self.encode_int_u64(*value as u64);

            } else if *value > std::u16::MAX as usize && *value <= U24_MAX {
                self.buf.put_u8(0xFD);
                self.encode_int_u24(*value as u32);

            } else if *value > std::u8::MAX as usize && *value <= std::u16::MAX as usize {
                self.buf.put_u8(0xFC);
                self.encode_int_u16(*value as u16);

            } else if *value <= std::u8::MAX as usize {
                match *value {
                    // If the value is of size u8 and one of the key bytes used in length encoding
                    // we must encode that single byte as a u16
                    0xFB | 0xFC | 0xFD | 0xFE | 0xFF => {
                        self.buf.put_u8(0xFC);
                        self.buf.put_u8(*value as u8);
                        self.buf.put_u8(0);
                    }

                    v => self.buf.put_u8(v as u8),
                }
            } else {
                panic!("Value is too long");
            }
        } else {
            self.buf.put_u8(0xFB);
        }
    }

    // Encode a string<lenenc>; a length encoded string.
    #[inline]
    pub fn encode_string_lenenc(&mut self, string: &Bytes) {
        if string.len() > 0xFFF {
            panic!("String inside string lenenc serialization is too long");
        }

        self.encode_int_lenenc(Some(&string.len()));
        if string.len() > 0 {
            self.buf.extend_from_slice(string);
        }
    }

    // Encode a string<null>; a null termianted string (C style)
    #[inline]
    pub fn encode_string_null(&mut self, string: &Bytes) {
        self.buf.extend_from_slice(string);
        self.buf.put(0_u8);
    }

    // Encode a string<fix>; a string of fixed length
    #[inline]
    pub fn encode_string_fix(&mut self, bytes: &Bytes, size: usize) {
        if size != bytes.len() {
            panic!("Sizes do not match");
        }

        self.buf.extend_from_slice(bytes);
    }

    // Encode a string<eof>; a string that is terminated by the packet length
    #[inline]
    pub fn encode_string_eof(&mut self, bytes: &Bytes) {
        self.buf.extend_from_slice(bytes);
    }

    // Same as the string counterpart copied to maintain consistency with the spec.
    #[inline]
    pub fn encode_byte_lenenc(&mut self, bytes: &Bytes) {
        if bytes.len() > 0xFFF {
            panic!("String inside string lenenc serialization is too long");
        }

        self.encode_int_lenenc(Some(&bytes.len()));
        if bytes.len() > 0 {
            self.buf.extend_from_slice(bytes);
        }
    }

    // Same as the string counterpart copied to maintain consistency with the spec.
    #[inline]
    pub fn encode_byte_fix(&mut self, bytes: &Bytes, size: usize) {
        if size != bytes.len() {
            panic!("Sizes do not match");
        }

        self.buf.extend_from_slice(bytes);
    }

    // Same as the string counterpart copied to maintain consistency with the spec.
    #[inline]
    pub fn encode_byte_eof(&mut self, bytes: &Bytes) {
        self.buf.extend_from_slice(bytes);
    }

    #[inline]
    pub fn encode_param(&mut self, bytes: &Bytes, ty: &FieldType) {
        match ty {
            FieldType::MysqlTypeDecimal => self.encode_string_lenenc(bytes),
            FieldType::MysqlTypeTiny => self.encode_int_1(bytes),
            FieldType::MysqlTypeShort => self.encode_int_2(bytes),
            FieldType::MysqlTypeLong => self.encode_int_4(bytes),
            FieldType::MysqlTypeFloat => self.encode_int_4(bytes),
            FieldType::MysqlTypeDouble => self.encode_int_8(bytes),
            FieldType::MysqlTypeNull => panic!("Type cannot be FieldType::MysqlTypeNull"),
            FieldType::MysqlTypeTimestamp => unimplemented!(),
            FieldType::MysqlTypeLonglong => self.encode_int_8(bytes),
            FieldType::MysqlTypeInt24 => self.encode_int_4(bytes),
            FieldType::MysqlTypeDate => unimplemented!(),
            FieldType::MysqlTypeTime => unimplemented!(),
            FieldType::MysqlTypeDatetime => unimplemented!(),
            FieldType::MysqlTypeYear => self.encode_int_4(bytes),
            FieldType::MysqlTypeNewdate => unimplemented!(),
            FieldType::MysqlTypeVarchar => self.encode_string_lenenc(bytes),
            FieldType::MysqlTypeBit => self.encode_string_lenenc(bytes),
            FieldType::MysqlTypeTimestamp2 => unimplemented!(),
            FieldType::MysqlTypeDatetime2 => unimplemented!(),
            FieldType::MysqlTypeTime2 =>unimplemented!(),
            FieldType::MysqlTypeJson => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeNewdecimal => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeEnum => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeSet => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeTinyBlob => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeMediumBlob => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeLongBlob => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeBlob => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeVarString => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeString => self.encode_byte_lenenc(bytes),
            FieldType::MysqlTypeGeometry => self.encode_byte_lenenc(bytes),
        }
    }
}

impl From<BytesMut> for Encoder {
    fn from(buf: BytesMut) -> Encoder {
        Encoder { buf }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // [X] it_encodes_int_lenenc_u64
    // [X] it_encodes_int_lenenc_u32
    // [X] it_encodes_int_lenenc_u24
    // [X] it_encodes_int_lenenc_u16
    // [X] it_encodes_int_lenenc_u8
    // [X] it_encodes_int_u64
    // [X] it_encodes_int_u32
    // [X] it_encodes_int_u24
    // [X] it_encodes_int_u16
    // [X] it_encodes_int_u8
    // [X] it_encodes_string_lenenc
    // [X] it_encodes_string_fix
    // [X] it_encodes_string_null
    // [X] it_encodes_string_eof
    // [X] it_encodes_byte_lenenc
    // [X] it_encodes_byte_fix
    // [X] it_encodes_byte_eof

    #[test]
    fn it_encodes_int_lenenc_none() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(None);

        assert_eq!(&encoder.buf[..], b"\xFB");
    }

    #[test]
    fn it_encodes_int_lenenc_u8() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(Some(&(0xFA as usize)));

        assert_eq!(&encoder.buf[..], b"\xFA");
    }

    #[test]
    fn it_encodes_int_lenenc_u16() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(Some(&(std::u16::MAX as usize)));

        assert_eq!(&encoder.buf[..], b"\xFC\xFF\xFF");
    }

    #[test]
    fn it_encodes_int_lenenc_u24() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(Some(&U24_MAX));

        assert_eq!(&encoder.buf[..], b"\xFD\xFF\xFF\xFF");
    }

    #[test]
    fn it_encodes_int_lenenc_u64() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(Some(&(std::u64::MAX as usize)));

        assert_eq!(&encoder.buf[..], b"\xFE\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF");
    }

    #[test]
    fn it_encodes_int_lenenc_fb() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(Some(&(0xFB as usize)));

        assert_eq!(&encoder.buf[..], b"\xFC\xFB\x00");
    }

    #[test]
    fn it_encodes_int_lenenc_fc() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(Some(&(0xFC as usize)));

        assert_eq!(&encoder.buf[..], b"\xFC\xFC\x00");
    }

    #[test]
    fn it_encodes_int_lenenc_fd() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(Some(&(0xFD as usize)));

        assert_eq!(&encoder.buf[..], b"\xFC\xFD\x00");
    }


    #[test]
    fn it_encodes_int_lenenc_fe() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(Some(&(0xFE as usize)));

        assert_eq!(&encoder.buf[..], b"\xFC\xFE\x00");
    }

    fn it_encodes_int_lenenc_ff() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_lenenc(Some(&(0xFF as usize)));

        assert_eq!(&encoder.buf[..], b"\xFC\xFF\x00");
    }

    #[test]
    fn it_encodes_int_u64() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_u64(std::u64::MAX);

        assert_eq!(&encoder.buf[..], b"\xFF\xFF\xFF\xFF\xFF\xFF\xFF\xFF");
    }

    #[test]
    fn it_encodes_int_u32() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_u32(std::u32::MAX);

        assert_eq!(&encoder.buf[..], b"\xFF\xFF\xFF\xFF");
    }

    #[test]
    fn it_encodes_int_u24() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_u24(U24_MAX as u32);

        assert_eq!(&encoder.buf[..], b"\xFF\xFF\xFF");
    }

    #[test]
    fn it_encodes_int_u16() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_u16(std::u16::MAX);

        assert_eq!(&encoder.buf[..], b"\xFF\xFF");
    }

    #[test]
    fn it_encodes_int_u8() {
        let mut encoder = Encoder::new(128);
        encoder.encode_int_u8(std::u8::MAX);

        assert_eq!(&encoder.buf[..], b"\xFF");
    }

    #[test]
    fn it_encodes_string_lenenc() {
        let mut encoder = Encoder::new(128);
        encoder.encode_string_lenenc(&Bytes::from_static(b"random_string"));

        assert_eq!(&encoder.buf[..], b"\x0Drandom_string");
    }

    #[test]
    fn it_encodes_string_fix() {
        let mut encoder = Encoder::new(128);
        encoder.encode_string_fix(&Bytes::from_static(b"random_string"), 13);

        assert_eq!(&encoder.buf[..], b"random_string");
    }

    #[test]
    fn it_encodes_string_null() {
        let mut encoder = Encoder::new(128);
        encoder.encode_string_null(&Bytes::from_static(b"random_string"));

        assert_eq!(&encoder.buf[..], b"random_string\0");
    }

    #[test]
    fn it_encodes_string_eof() {
        let mut encoder = Encoder::new(128);
        encoder.encode_string_eof(&Bytes::from_static(b"random_string"));

        assert_eq!(&encoder.buf[..], b"random_string");
    }

    #[test]
    fn it_encodes_byte_lenenc() {
        let mut encoder = Encoder::new(128);
        encoder.encode_byte_lenenc(&Bytes::from("random_string"));

        assert_eq!(&encoder.buf[..], b"\x0D\x00\x00random_string");
    }

    #[test]
    fn it_encodes_byte_fix() {
        let mut encoder = Encoder::new(128);
        encoder.encode_byte_fix(&Bytes::from("random_string"), 13);

        assert_eq!(&encoder.buf[..], b"random_string");
    }

    #[test]
    fn it_encodes_byte_eof() {
        let mut encoder = Encoder::new(128);
        encoder.encode_byte_eof(&Bytes::from("random_string"));

        assert_eq!(&encoder.buf[..], b"random_string");
    }
}
