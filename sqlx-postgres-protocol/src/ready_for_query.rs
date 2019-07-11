use crate::Decode;
use bytes::Bytes;
use std::io;

#[derive(Debug, PartialEq, Clone, Copy)]
#[repr(u8)]
pub enum TransactionStatus {
    /// Not in a transaction block.
    Idle = b'I',

    /// In a transaction block.
    Transaction = b'T',

    /// In a _failed_ transaction block. Queries will be rejected until block is ended.
    Error = b'E',
}

/// `ReadyForQuery` is sent whenever the backend is ready for a new query cycle.
#[derive(Debug)]
pub struct ReadyForQuery {
    pub status: TransactionStatus,
}

impl Decode for ReadyForQuery {
    fn decode(src: Bytes) -> io::Result<Self> {
        if src.len() != 1 {
            return Err(io::ErrorKind::InvalidInput)?;
        }

        Ok(Self {
            status: match src[0] {
                // FIXME: Variant value are duplicated with declaration
                b'I' => TransactionStatus::Idle,
                b'T' => TransactionStatus::Transaction,
                b'E' => TransactionStatus::Error,

                _ => unreachable!(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{ReadyForQuery, TransactionStatus};
    use crate::Decode;
    use bytes::Bytes;
    use std::io;

    const READY_FOR_QUERY: &[u8] = b"E";

    #[test]
    fn it_decodes_ready_for_query() -> io::Result<()> {
        let src = Bytes::from_static(READY_FOR_QUERY);
        let message = ReadyForQuery::decode(src)?;

        assert_eq!(message.status, TransactionStatus::Error);

        Ok(())
    }
}
