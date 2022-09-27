use crate::ws::Channel;
use thiserror::Error;
use tokio_tungstenite::tungstenite;

#[derive(Debug, Error)]
pub enum WsError {
    #[error("Orderbook checksum was not correct")]
    IncorrectChecksum,

    #[error("Orderbook has not yet received partial")]
    MissingPartial,

    #[error("Not subscribed to this channel {0:?}")]
    NotSubscribedToThisChannel(Channel),

    #[error("Missing subscription confirmation")]
    MissingSubscriptionConfirmation,

    #[error("Socket is not authenticated")]
    SocketNotAuthenticated,

    #[error(transparent)]
    Tungstenite(#[from] tungstenite::Error),

    #[error(transparent)]
    Serde(#[from] serde_json::Error),

    #[error(transparent)]
    SystemTime(#[from] std::time::SystemTimeError),
}
