pub mod client;
pub mod decode;
pub mod deserialize;
pub mod encode;
pub mod error_codes;
pub mod packets;
pub mod serialize;
pub mod server;
pub mod types;

// Re-export all the things
pub use packets::AuthenticationSwitchRequestPacket;
pub use packets::ColumnPacket;
pub use packets::ColumnDefPacket;
pub use packets::ComDebug;
pub use packets::ComInitDb;
pub use packets::ComPing;
pub use packets::ComProcessKill;
pub use packets::ComQuery;
pub use packets::ComQuit;
pub use packets::ComResetConnection;
pub use packets::ComSetOption;
pub use packets::SetOptionOptions;
pub use packets::ShutdownOptions;
pub use packets::ComShutdown;
pub use packets::ComSleep;
pub use packets::ComStatistics;
pub use packets::EofPacket;
pub use packets::ErrPacket;
pub use packets::HandshakeResponsePacket;
pub use packets::InitialHandshakePacket;
pub use packets::OkPacket;
pub use packets::PacketHeader;
pub use packets::ResultSet;
pub use packets::ResultRow;
pub use packets::SSLRequestPacket;

pub use decode::Decoder;

pub use deserialize::DeContext;
pub use deserialize::Deserialize;

pub use encode::Encoder;

pub use error_codes::ErrorCode;

pub use serialize::Serialize;

pub use server::Message;

pub use types::Capabilities;
pub use types::ServerStatusFlag;
pub use types::FieldType;
pub use types::FieldDetailFlag;
pub use types::SessionChangeType;

pub use client::TextProtocol;
