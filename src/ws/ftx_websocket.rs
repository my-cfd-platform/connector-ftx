use futures::{
    ready,
    task::{Context, Poll},
    Future, SinkExt, Stream, StreamExt,
};
use hmac_sha256::HMAC;
use serde_json::json;
use std::collections::VecDeque;
use std::pin::Pin;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use super::{Channel, Data, Error, Symbol};

pub struct FtxWebsocket {
    channels: Vec<Channel>,
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    buf: VecDeque<(Option<Symbol>, Data)>,
    ping_time: Duration,
    /// Whether the websocket was opened authenticated with API keys or not
    is_authenticated: bool,
}

impl FtxWebsocket {
    pub const ENDPOINT: &'static str = "wss://ftx.com/ws";

    pub async fn connect(
        key: Option<String>,
        secret: Option<String>,
        subaccount: Option<String>,
    ) -> Result<Self, Error> {
        let (mut stream, _) = connect_async(FtxWebsocket::ENDPOINT).await?;
        
        let is_authenticated = if let (Some(key), Some(secret)) = (key, secret) {
            let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
            let sign_payload = format!("{}websocket_login", timestamp);
            let sign = HMAC::mac(sign_payload.as_bytes(), secret.as_bytes());
            let sign = hex::encode(sign);

            stream
                .send(Message::Text(
                    json!({
                        "op": "login",
                        "args": {
                            "key": key,
                            "sign": sign,
                            "time": timestamp as u64,
                            "subaccount": subaccount,
                        }
                    })
                    .to_string(),
                ))
                .await?;
            true
        } else {
            false
        };

        Ok(Self {
            channels: Vec::new(),
            stream,
            buf: VecDeque::new(),
            ping_time: Duration::from_secs(15),
            is_authenticated,
        })
    }

    async fn ping(&mut self) -> Result<(), Error> {
        self.stream
            .send(Message::Text(
                json!({
                    "op": "ping",
                })
                .to_string(),
            ))
            .await?;

        Ok(())
    }
}

