use super::super::{client::TextProtocol, serialize::Serialize};
use crate::connection::Connection;
use failure::Error;

#[derive(Clone, Copy)]
pub enum ShutdownOptions {
    ShutdownDefault = 0x00,
}

pub struct ComShutdown {
    pub option: ShutdownOptions,
}

impl Serialize for ComShutdown {
    fn serialize<'a, 'b>(&self, ctx: &mut crate::connection::ConnContext, encoder: &mut crate::protocol::encode::Encoder) -> Result<(), Error> {
        encoder.encode_int_1(TextProtocol::ComShutdown.into());
        encoder.encode_int_1(self.option.into());

        Ok(())
    }
}
