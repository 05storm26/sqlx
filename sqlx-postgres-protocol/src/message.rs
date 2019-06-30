use crate::{Authentication, BackendKeyData, Decode, ParameterStatus, ReadyForQuery, Response};
use byteorder::{BigEndian, ByteOrder};
use bytes::BytesMut;
use std::io;

#[derive(Debug)]
pub enum Message {
    Authentication(Authentication),
    ParameterStatus(ParameterStatus),
    BackendKeyData(BackendKeyData),
    ReadyForQuery(ReadyForQuery),
    Response(Box<Response>),
}

impl Message {
    // FIXME: `Message::decode` shares the name of the remaining message type `::decode` despite being very
    //        different
    pub fn decode(src: &mut BytesMut) -> io::Result<Option<Self>>
    where
        Self: Sized,
    {
        if src.len() < 5 {
            // No message is less than 5 bytes
            return Ok(None);
        }

        let token = src[0];
        if token == 0 {
            // FIXME: Handle end-of-stream
            return Err(io::ErrorKind::InvalidData)?;
        }

        // FIXME: What happens if len(u32) < len(usize) ?
        let len = BigEndian::read_u32(&src[1..5]) as usize;

        if src.len() < len {
            // We don't have enough in the stream yet
            return Ok(None);
        }

        let src = src.split_to(len + 1).freeze().slice_from(5);

        Ok(Some(match token {
            b'N' | b'E' => Message::Response(Box::new(Response::decode(src)?)),
            b'S' => Message::ParameterStatus(ParameterStatus::decode(src)?),
            b'Z' => Message::ReadyForQuery(ReadyForQuery::decode(src)?),
            b'R' => Message::Authentication(Authentication::decode(src)?),
            b'K' => Message::BackendKeyData(BackendKeyData::decode(src)?),

            _ => unimplemented!("decode not implemented for token: {}", token as char),
        }))
    }
}
