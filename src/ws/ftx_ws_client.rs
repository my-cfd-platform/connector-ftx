use my_web_socket_client::WebSocketClient;
use my_web_socket_client::WsCallback;
use my_web_socket_client::WsConnection;
use rust_extensions::Logger;
use serde_json::json;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use tokio_tungstenite::tungstenite::Message;

use crate::ws::WsMessageType;

use super::event_handler::*;
use super::ftx_ws_settings::FtxWsSetting;
use super::models::*;

pub struct FtxWsClient {
    event_handler: Arc<dyn EventHandler + Send + Sync + 'static>,
    ws_client: WebSocketClient,
    channels: Vec<WsChannel>,
    logger: Arc<dyn Logger + Send + Sync + 'static>,
    is_started: AtomicBool,
}

impl FtxWsClient {
    pub fn new(
        event_handler: Arc<dyn EventHandler + Send + Sync + 'static>,
        logger: Arc<dyn Logger + Send + Sync + 'static>,
        channels: Vec<WsChannel>,
    ) -> Self {
        let settings = Arc::new(FtxWsSetting::new());
        Self {
            event_handler,
            ws_client: WebSocketClient::new("FTX".to_string(), settings, logger.clone()),
            channels,
            logger,
            is_started: AtomicBool::new(false),
        }
    }

    async fn subscribe_or_unsubscribe(
        &self,
        ws_connection: Arc<WsConnection>,
        channels: Vec<WsChannel>,
        subscribe: bool,
    ) {
        let op = if subscribe {
            "subscribe"
        } else {
            "unsubscribe"
        };

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
        if !ftx_ws_client
            .is_started
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            let ping_message = Message::Text(
                json!({
                    "op": "ping",
                })
                .to_string(),
            );
            ftx_ws_client
                .ws_client
                .start(ping_message, ftx_ws_client.clone());
            ftx_ws_client
                .is_started
                .store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }
}

#[async_trait::async_trait]
impl WsCallback for FtxWsClient {
    async fn on_connected(&self, ws_connection: Arc<WsConnection>) {
        self.logger.write_info(
            "FtxWsClient".to_string(),
            "Connected to FTX websocket".to_string(),
            None,
        );

        self.subscribe_or_unsubscribe(ws_connection, self.channels.clone(), true)
            .await; 
    }

    async fn on_disconnected(&self, _: Arc<WsConnection>) {}

    async fn on_data(&self, connection: Arc<WsConnection>, data: Message) {
        if let Message::Text(text) = data {
            let result: Result<WsResponse, _> = serde_json::from_str(&text);

            if result.is_err() {
                self.logger.write_error(
                    "FtxWsClient".to_string(),
                    format!("Failed to parce message: {}", text),
                    None,
                );
                connection.disconnect().await;
                return;
            }

            let response = result.unwrap();

            match response.r#type {
                WsMessageType::Subscribed => {
                    self.logger.write_info(
                        "FtxWsClient".to_string(),
                        format!("Subscribed to FTX channel {}", response.market.unwrap_or_default()),
                        None,
                    );
                },
                WsMessageType::Unsubscribed
                | WsMessageType::Pong
                | WsMessageType::Info => return,
                WsMessageType::Error => {
                    self.logger.write_error(
                        "FtxWsClient".to_string(),
                        format!("Disconnecting... Recieved error: {:?}", response),
                        None,
                    );
                    connection.disconnect().await;
                }
                WsMessageType::Partial | WsMessageType::Update => {
                    self.event_handler.on_data(WsDataEvent::new(response)).await
                }
            }
        }
    }
}
