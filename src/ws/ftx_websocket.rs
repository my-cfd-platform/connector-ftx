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
use tokio::{net::TcpStream, time};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tokio::time::Interval;

use crate::ws::WsMessageType;

use super::{Channel, Data, Error, Symbol, WsResponseData, WsResponse};

pub struct FtxWebsocket {
    channels: Vec<Channel>,
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    buf: VecDeque<(Option<Symbol>, Data)>,
    ping_timer: Interval,
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
            ping_timer: time::interval(Duration::from_secs(15)),
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

    pub async fn subscribe(&mut self, channels: &[Channel]) -> Result<(), Error> {
        for channel in channels.iter() {
            // Subscribing to fills or orders requires us to be authenticated via an API key
            if (channel == &Channel::Fills || channel == &Channel::Orders) && !self.is_authenticated
            {
                return Err(Error::SocketNotAuthenticated);
            }
            self.channels.push(channel.clone());
        }

        self.subscribe_or_unsubscribe(channels, true).await?;

        Ok(())
    }

    pub async fn unsubscribe(&mut self, channels: &[Channel]) -> Result<(), Error> {
        // Check that the specified channels match an existing one
        for channel in channels.iter() {
            if !self.channels.contains(channel) {
                return Err(Error::NotSubscribedToThisChannel(channel.clone()));
            }
        }

        self.subscribe_or_unsubscribe(channels, false).await?;
        self.channels.retain(|c| !channels.contains(c));

        Ok(())
    }

    pub async fn unsubscribe_all(&mut self) -> Result<(), Error> {
        let channels = self.channels.clone();
        self.unsubscribe(&channels).await?;

        self.channels.clear();

        Ok(())
    }

    async fn subscribe_or_unsubscribe(
        &mut self,
        channels: &[Channel],
        subscribe: bool,
    ) -> Result<(), Error> {
        let op = if subscribe {
            "subscribe"
        } else {
            "unsubscribe"
        };

        'channels: for channel in channels {
            let (channel, symbol) = match channel {
                Channel::Orderbook(symbol) => ("orderbook", symbol.as_str()),
                Channel::Trades(symbol) => ("trades", symbol.as_str()),
                Channel::Ticker(symbol) => ("ticker", symbol.as_str()),
                Channel::Fills => ("fills", ""),
                Channel::Orders => ("orders", ""),
            };

            self.stream
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
                        // Subscribe confirmed
                        continue 'channels;
                    }
                    WsResponse {
                        r#type: WsMessageType::Unsubscribed,
                        ..
                    } if !subscribe => {
                        // Unsubscribe confirmed
                        continue 'channels;
                    }
                    _ => {
                        // Otherwise, continue adding contents to buffer
                        self.add_to_buffer(response);
                    }
                }
            }

            return Err(Error::MissingSubscriptionConfirmation);
        }

        Ok(())
    }

    async fn next_response(&mut self) -> Result<WsResponse, Error> {
        loop {
            tokio::select! {
                _ = self.ping_timer.tick() => {
                    self.ping().await?;
                },
                Some(msg) = self.stream.next() => {
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

    fn add_to_buffer(&mut self, response: WsResponse) {
        if let Some(data) = response.data {
            match data {
                WsResponseData::Trades(trades) => {
                    for trade in trades {
                        self.buf
                            .push_back((response.market.clone(), Data::Trade(trade)));
                    }
                }
                WsResponseData::OrderbookData(orderbook) => {
                    self.buf
                        .push_back((response.market, Data::OrderbookData(orderbook)));
                }
                WsResponseData::Fill(fill) => {
                    self.buf.push_back((response.market, Data::Fill(fill)));
                }
                WsResponseData::Ticker(ticker) => {
                    self.buf.push_back((response.market, Data::Ticker(ticker)));
                }
                WsResponseData::Order(order) => {
                    self.buf.push_back((response.market, Data::Order(order)));
                }
            }
        }
    }
}