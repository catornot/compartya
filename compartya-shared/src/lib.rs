use std::net::SocketAddr;
use thiserror::Error;

pub type LobbyUid = [char; 8];
pub type Password = [char; 8];
pub type PlayerUid = [char; 5];

#[derive(serde::Deserialize, serde::Serialize, Debug, Clone, Default)]
pub enum Order {
    JoinServer(String, String),

    #[default]
    LeaveServer,
}

#[derive(Error, Debug)]
pub enum PartyaError {
    #[error("illegal uid {0:?}")]
    IllegalUid(PlayerUid, SocketAddr),

    #[error("the stun server got an illegal packet {0:?}")]
    IllegalPacket(Box<SentPacket>),

    #[error("{0}")]
    Any(String),

    #[error(transparent)]
    BinCodeError(#[from] bincode::Error),
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub enum SentPacket {
    PacketMessage(PacketMessage),
    PacketResponse(PacketResponse),
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub enum PacketMessage {
    // server
    FindLobby(LobbyUid),
    CreateLobby,
    NewClient(SocketAddr),

    // plugin
    Auth(Password),
    GetLastOrder(PlayerUid),
    NewOrder(PlayerUid, Order),
    VibeCheck,

    // general
    Ping(Option<PlayerUid>),
}

#[derive(serde::Deserialize, serde::Serialize, Debug)]
pub enum PacketResponse {
    FoundLobby(SocketAddr),
    NoLobby(LobbyUid),
    CreatedLobby(LobbyUid),

    // plugin
    AuthAccepted(PlayerUid, Password),
    FailedAuth,

    // general
    Pong,
}

impl<'a> TryFrom<&'a [u8]> for SentPacket {
    type Error = bincode::Error;

    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        bincode::deserialize(value)
    }
}

impl<'a> TryInto<Vec<u8>> for &'a SentPacket {
    type Error = bincode::Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        bincode::serialize(self)
    }
}

impl TryInto<Vec<u8>> for SentPacket {
    type Error = bincode::Error;

    fn try_into(self) -> Result<Vec<u8>, Self::Error> {
        bincode::serialize(&self)
    }
}

impl Into<SentPacket> for PacketMessage {
    fn into(self) -> SentPacket {
        SentPacket::PacketMessage(self)
    }
}

impl Into<SentPacket> for PacketResponse {
    fn into(self) -> SentPacket {
        SentPacket::PacketResponse(self)
    }
}

impl PacketMessage {
    pub fn send(self) -> SentPacket {
        SentPacket::PacketMessage(self)
    }
}

impl PacketResponse {
    pub fn send(self) -> SentPacket {
        SentPacket::PacketResponse(self)
    }
}
