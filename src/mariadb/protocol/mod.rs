// Reference: https://mariadb.com/kb/en/library/connection
// Packets: https://mariadb.com/kb/en/library/0-packet

mod binary;
mod capabilities;
mod connect;
mod encode;
mod error_code;
mod field;
mod response;
mod server_status;
mod text;

pub use binary::{
    ComStmtClose, ComStmtExec, ComStmtFetch, ComStmtPrepare, ComStmtPrepareOk, ComStmtReset,
};
pub use capabilities::Capabilities;
pub use connect::{
    AuthenticationSwitchRequest, HandshakeResponsePacket, InitialHandshakePacket, SslRequest,
};
pub use encode::Encode;
pub use error_code::ErrorCode;
pub use field::{FieldDetailFlag, FieldType, ParameterFlag};
pub use response::{
    ColumnCountPacket, ColumnDefinitionPacket, EofPacket, ErrPacket, OkPacket, ResultRow,
};
pub use server_status::ServerStatusFlag;
pub use text::{ComDebug, ComInitDb, ComPing, ComProcessKill, ComQuery, ComQuit};
