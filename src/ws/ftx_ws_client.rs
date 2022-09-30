use rust_extensions::Logger;
use serde_json::json;
use std::sync::Arc;

use tokio_tungstenite::tungstenite::Message;

use crate::ws_client::{WebSocketClient, WsCallback, WsClientSettings, WsConnection};
use crate::{ws::WsMessageType};

use super::event_handler::*;
use super::models::*;

pub struct FtxWsClient {
    event_handler: Arc<dyn EventHandler + Send + Sync + 'static>,
    ws_client: WebSocketClient,
    channels: Vec<WsChannel>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
}

impl FtxWsClient {
    pub fn new(
        event_handler: Arc<dyn EventHandler + Send + Sync + 'static>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        settings: Arc<dyn WsClientSettings + Send + Sync + 'static>,
        channels: Vec<WsChannel>,
    ) -> Self {
        Self {
            event_handler,
            ws_client: WebSocketClient::new("FTX".to_string(), settings, logger.clone()),
            channels,
            logger
        }
    }

    async fn subscribe_or_unsubscribe(&self, ws_connection: Arc<WsConnection>, subscribe: bool) {
        let op = if subscribe {
            "subscribe"
        } else {
            "unsubscribe"
        };
        let channels = self.channels.clone();

        for channel in channels {
            let (channel, symbol) = match channel {
                WsChannel::Orderbook(symbol) => ("orderbook", symbol),
                WsChannel::Trades(symbol) => ("trades", symbol),
                WsChannel::Ticker(symbol) => ("ticker", symbol),
                WsChannel::Fills => ("fills", "".to_string()),
                WsChannel::Orders => ("orders", "".to_string()),
            };

            ws_connection
                .send_message(Message::Text(
                    json!({
                        "op": op,
                        "channel": channel,
                        "market": symbol,
                    })
                    .to_string(),
                ))
                .await;
        }
    }

    pub fn start(ftx_ws_client: Arc<FtxWsClient>) {
        let ping_message = Message::Text(
            json!({
                "op": "ping",
            })
            .to_string(),
        );
        ftx_ws_client
            .ws_client
            .start(ping_message, ftx_ws_client.clone());
    }
}

#[async_trait::async_trait]
impl WsCallback for FtxWsClient {
    async fn on_connected(&self, ws_connection: Arc<WsConnection>) {
        self.subscribe_or_unsubscribe(ws_connection, true).await;
    }

    async fn on_disconnected(&self, _: Arc<WsConnection>) {}

    async fn on_data(&self, _: Arc<WsConnection>, data: Message) {
        if let Message::Text(text) = data {
            let result: Result<WsResponse, _> = serde_json::from_str(&text);

            if result.is_err() {
                self.logger.write_error("FtxWsClient".to_string(), format!("Failed to parce message: {}", text), None)
            }
            
            let response = result.unwrap();

            match response.r#type {
                WsMessageType::Subscribed
                | WsMessageType::Unsubscribed
                | WsMessageType::Pong
                | WsMessageType::Info => return,
                WsMessageType::Error => self.logger.write_error("FtxWsClient".to_string(), format!("Reciveved error: {:?}", response), None),
                WsMessageType::Partial | WsMessageType::Update => {
                    self.event_handler.on_data(WsDataEvent::new(response)).await
                }
            }
        }
    }
}
