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

use super::{WsChannel, EventData, WsError, Symbol, WsResponseData, WsResponse};

pub struct FtxWebsocket {
    channels: Vec<WsChannel>,
    stream: WebSocketStream<MaybeTlsStream<TcpStream>>,
    buf: VecDeque<(Option<Symbol>, EventData)>,
    ping_timer: Interval,
    is_authenticated: bool,
}

impl FtxWebsocket {
    pub const ENDPOINT: &'static str = "wss://ftx.com/ws";

    pub async fn connect(
        key: Option<String>,
        secret: Option<String>,
        subaccount: Option<String>,
    ) -> Result<Self, WsError> {
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

    async fn ping(&mut self) -> Result<(), WsError> {
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

    pub async fn subscribe(&mut self, channels: &[WsChannel]) -> Result<(), WsError> {
        for channel in channels.iter() {
            if (channel == &WsChannel::Fills || channel == &WsChannel::Orders) && !self.is_authenticated
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
                        self.add_to_buffer(response);
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
                            .push_back((response.market.clone(), EventData::Trade(trade)));
                    }
                }
                WsResponseData::OrderbookData(orderbook) => {
                    self.buf
                        .push_back((response.market, EventData::OrderbookData(orderbook)));
                }
                WsResponseData::Fill(fill) => {
                    self.buf.push_back((response.market, EventData::Fill(fill)));
                }
                WsResponseData::Ticker(ticker) => {
                    self.buf.push_back((response.market, EventData::Ticker(ticker)));
                }
                WsResponseData::Order(order) => {
                    self.buf.push_back((response.market, EventData::Order(order)));
                }
            }
        }
    }
}

impl Stream for FtxWebsocket {
    type Item = Result<(Option<Symbol>, EventData), WsError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if let Some(data) = self.buf.pop_front() {
                return Poll::Ready(Some(Ok(data)));
            }
            
            // Fetch new response if buffer is empty
            let response = {
                // safety: this is ok because the future from self.next_response() will only live in this function.
                // It won't be moved anymore.
                let mut next_response = self.next_response();
                let pinned = unsafe { Pin::new_unchecked(&mut next_response) };
                match ready!(pinned.poll(cx)) {
                    Ok(response) => response,
                    Err(e) => {
                        return Poll::Ready(Some(Err(e)));
                    }
                }
            };

            self.add_to_buffer(response);
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.buf.len(), None)
    }
}