use futures::{
    SinkExt, StreamExt,
};
use serde_json::json;
use std::time::Duration;
use std::{sync::Arc};
use tokio::time::Interval;
use tokio::{net::TcpStream, time};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};

use crate::{ftx_auth_settings::FtxAuthSettings, ws::WsMessageType};

use super::error::*;
use super::event_handler::*;
use super::models::*;

pub struct FtxWsClient {
    channels: Vec<WsChannel>,
    stream: Option<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    ping_timer: Interval,
    event_handler: Arc<dyn EventHandler + Send + Sync + 'static>,
    auth_settings: Option<FtxAuthSettings>,
    is_authenticated: bool,
}

impl FtxWsClient {
    pub const ENDPOINT: &'static str = "wss://ftx.com/ws";

    pub fn new(
        event_handler: Arc<dyn EventHandler + Send + Sync + 'static>,
        auth_settings: Option<FtxAuthSettings>,
    ) -> Self {
        Self {
            channels: Vec::new(),
            stream: None,
            ping_timer: time::interval(Duration::from_secs(15)),
            is_authenticated: false,
            event_handler,
            auth_settings,
        }
    }

    pub async fn connect(&mut self) -> Result<(), WsError> {
        let (stream, _) = connect_async(FtxWsClient::ENDPOINT).await?;
        self.stream = Some(stream);

        if let Some(_) = self.auth_settings {
            self.authenticate().await;
            self.is_authenticated = true;
        }

        Ok(())
    }

    async fn authenticate(&mut self) {
        let timestamp = FtxAuthSettings::generate_timestamp();
        let auth_settings = self.auth_settings.as_ref().unwrap();
        let sign = auth_settings.generate_sign("websocket_login", timestamp);

        self.stream
            .as_mut()
            .unwrap()
            .send(Message::Text(
                json!({
                    "op": "login",
                    "args": {
                        "key": auth_settings.api_key,
                        "sign": sign,
                        "time": timestamp as u64,
                        "subaccount": auth_settings.subaccount,
                    }
                })
                .to_string(),
            ))
            .await
            .unwrap();
    }

    async fn ping(&mut self) -> Result<(), WsError> {
        self.stream
            .as_mut()
            .unwrap()
            .send(Message::Text(
                json!({
                    "op": "ping",
                })
                .to_string(),
            ))
            .await?;

        Ok(())
    }

    pub async fn subscribe(&mut self, channels: &[WsChannel]) -> Result<(), WsError> {
        for channel in channels.iter() {
            if (channel == &WsChannel::Fills || channel == &WsChannel::Orders)
                && !self.is_authenticated
            {
                return Err(WsError::SocketNotAuthenticated);
            }
            self.channels.push(channel.clone());
        }

        self.subscribe_or_unsubscribe(channels, true).await?;

        Ok(())
    }

    pub async fn unsubscribe(&mut self, channels: &[WsChannel]) -> Result<(), WsError> {
        for channel in channels.iter() {
            if !self.channels.contains(channel) {
                return Err(WsError::NotSubscribedToThisChannel(channel.clone()));
            }
        }

        self.subscribe_or_unsubscribe(channels, false).await?;
        self.channels.retain(|c| !channels.contains(c));

        Ok(())
    }

    pub async fn unsubscribe_all(&mut self) -> Result<(), WsError> {
        let channels = self.channels.clone();
        self.unsubscribe(&channels).await?;

        self.channels.clear();

        Ok(())
    }

    async fn subscribe_or_unsubscribe(
        &mut self,
        channels: &[WsChannel],
        subscribe: bool,
    ) -> Result<(), WsError> {
        let op = if subscribe {
            "subscribe"
        } else {
            "unsubscribe"
        };

        'channels: for channel in channels {
            let (channel, symbol) = match channel {
                WsChannel::Orderbook(symbol) => ("orderbook", symbol.as_str()),
                WsChannel::Trades(symbol) => ("trades", symbol.as_str()),
                WsChannel::Ticker(symbol) => ("ticker", symbol.as_str()),
                WsChannel::Fills => ("fills", ""),
                WsChannel::Orders => ("orders", ""),
            };

            self.stream
                .as_mut()
                .unwrap()
                .send(Message::Text(
                    json!({
                        "op": op,
                        "channel": channel,
                        "market": symbol,
                    })
                    .to_string(),
                ))
                .await?;

            // Confirmation should arrive within the next 100 updates
            for _ in 0..100 {
                let response = self.next_response().await?;
                match response {
                    WsResponse {
                        r#type: WsMessageType::Subscribed,
                        ..
                    } if subscribe => {
                        continue 'channels;
                    }
                    WsResponse {
                        r#type: WsMessageType::Unsubscribed,
                        ..
                    } if !subscribe => {
                        continue 'channels;
                    }
                    _ => {
                        self.event_handler.on_data(WsDataEvent::new(response));
                    }
                }
            }

            return Err(WsError::MissingSubscriptionConfirmation);
        }

        Ok(())
    }

    async fn next_response(&mut self) -> Result<WsResponse, WsError> {
        loop {
            tokio::select! {
                _ = self.ping_timer.tick() => {
                    self.ping().await?;
                },
                Some(msg) = self.stream.as_mut().unwrap().next() => {
                    let msg = msg?;
                    if let Message::Text(text) = msg {
                        let response: WsResponse = serde_json::from_str(&text)?;

                        if let WsResponse { r#type: WsMessageType::Pong, .. } = response {
                            continue;
                        }

                        return Ok(response)
                    }
                },
            }
        }
    }

    pub fn start(self) {
        tokio::spawn(event_loop(self));
    }
}

async fn event_loop(mut ws: FtxWsClient) {
    loop {
        let resp = ws.next_response().await.expect("No data received");

        ws.event_handler.on_data(WsDataEvent::new(resp));
    }
}