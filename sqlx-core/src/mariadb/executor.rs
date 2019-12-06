use super::MariaDb;
use crate::{
    backend::Backend,
    describe::{Describe, ResultField},
    executor::Executor,
    mariadb::{
        protocol::{
            Capabilities, ColumnCountPacket, ColumnDefinitionPacket, ComStmtExecute, EofPacket,
            ErrPacket, OkPacket, ResultRow, StmtExecFlag,
        },
        query::MariaDbQueryParameters,
    },
    params::{IntoQueryParameters, QueryParameters},
    row::FromRow,
    url::Url,
};
use futures_core::{future::BoxFuture, stream::BoxStream};

impl Executor for MariaDb {
    type Backend = Self;

    fn execute<'e, 'q: 'e>(
        &'e mut self,
        query: &'q str,
        params: MariaDbQueryParameters,
    ) -> BoxFuture<'e, crate::Result<u64>> {
        Box::pin(async move {
            let statement_id = self.prepare_ignore_describe(query).await?;
            self.send_execute(statement_id, params).await?;

            let columns = self.result_column_defs().await?;
            let capabilities = self.capabilities;

            // For each row in the result set we will receive a ResultRow packet.
            // We may receive an [OkPacket], [EofPacket], or [ErrPacket] (depending on if EOFs are enabled) to finalize the iteration.
            let mut rows = 0u64;
            loop {
                let packet = self.receive().await?;
                if packet[0] == 0xFE && packet.len() < 0xFF_FF_FF {
                    // NOTE: It's possible for a ResultRow to start with 0xFE (which would normally signify end-of-rows)
                    //       but it's not possible for an Ok/Eof to be larger than 0xFF_FF_FF.
                    if !capabilities.contains(Capabilities::CLIENT_DEPRECATE_EOF) {
                        let _eof = EofPacket::decode(packet)?;
                    } else {
                        let _ok = OkPacket::decode(packet, capabilities)?;
                    }

                    break;
                } else if packet[0] == 0xFF {
                    let err = ErrPacket::decode(packet)?;
                    panic!("received db err = {:?}", err);
                } else {
                    // Ignore result rows; exec only returns number of affected rows;
                    let _ = ResultRow::decode(packet, &columns)?;

                    // For every row we decode we increment counter
                    rows = rows + 1;
                }
            }

            Ok(rows)
        })
    }

    fn fetch<'e, 'q: 'e, T: 'e>(
        &'e mut self,
        query: &'q str,
        params: MariaDbQueryParameters,
    ) -> BoxStream<'e, crate::Result<T>>
    where
        T: FromRow<Self::Backend> + Send + Unpin,
    {
        Box::pin(async_stream::try_stream! {
           let prepare = self.prepare_ignore_describe(query).await?;
           self.send_execute(prepare, params).await?;

           let columns = self.result_column_defs().await?;
           let capabilities = self.capabilities;

           loop {
               let packet = self.receive().await?;
               if packet[0] == 0xFE && packet.len() < 0xFF_FF_FF {
                   // NOTE: It's possible for a ResultRow to start with 0xFE (which would normally signify end-of-rows)
                   //       but it's not possible for an Ok/Eof to be larger than 0xFF_FF_FF.
                   if !capabilities.contains(Capabilities::CLIENT_DEPRECATE_EOF) {
                       let _eof = EofPacket::decode(packet)?;
                   } else {
                       let _ok = OkPacket::decode(packet, capabilities)?;
                   }

                   break;
               } else if packet[0] == 0xFF {
                   let _err = ErrPacket::decode(packet)?;
                   panic!("ErrPacket received");
               } else {
                   let row = ResultRow::decode(packet, &columns)?;
                   yield FromRow::from_row(row);
               }
           }
        })
    }

    fn fetch_optional<'e, 'q: 'e, T: 'e>(
        &'e mut self,
        query: &'q str,
        params: MariaDbQueryParameters,
    ) -> BoxFuture<'e, crate::Result<Option<T>>>
    where
        T: FromRow<Self::Backend> + Send,
    {
        Box::pin(async move {
            let statement_id = self.prepare_ignore_describe(query).await?;
            self.send_execute(statement_id, params).await?;

            let columns = self.result_column_defs().await?;
            let capabilities = self.capabilities;

            let mut row = None;

            loop {
                let packet = self.receive().await?;

                if packet[0] == 0xFE && packet.len() < 0xFF_FF_FF {
                    // NOTE: It's possible for a ResultRow to start with 0xFE (which would normally signify end-of-rows)
                    //       but it's not possible for an Ok/Eof to be larger than 0xFF_FF_FF.
                    if !capabilities.contains(Capabilities::CLIENT_DEPRECATE_EOF) {
                        let _eof = EofPacket::decode(packet)?;
                    } else {
                        let _ok = OkPacket::decode(packet, capabilities)?;
                    }

                    break;
                } else if packet[0] == 0xFF {
                    let _err = ErrPacket::decode(packet)?;
                    panic!("Received error packet: {:?}", _err);
                } else {
                    row = Some(FromRow::from_row(ResultRow::decode(packet, &columns)?));
                }
            }

            Ok(row)
        })
    }

    fn describe<'e, 'q: 'e>(
        &'e mut self,
        query: &'q str,
    ) -> BoxFuture<'e, crate::Result<Describe<Self::Backend>>> {
        Box::pin(self.prepare_describe(query))
    }
}
