pub use crate::common::{Coin, Id, MarketType, OrderInfo, Side, Symbol, TradeInfo};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, TimestampSecondsWithFrac};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WsChannel {
    Orderbook(String),
    Trades(String),
    Ticker(String),
    Fills,
    Orders,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsResponse {
    pub market: Option<String>,
    pub r#type: WsMessageType,
    pub data: Option<WsResponseData>,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WsMessageType {
    Subscribed,
    Unsubscribed,
    Update,
    Error,
    Partial,
    Pong,
    Info,
}

/// Represents the response received from FTX, and is used for
/// deserialization
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum WsResponseData {
    Ticker(TickerInfo),
    Trades(Vec<TradeInfo>),
    OrderbookData(OrderbookInfo),
    Fill(FillInfo),
    Order(OrderInfo),
}

#[serde_as]
#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TickerInfo {
    pub bid: Decimal,
    pub ask: Decimal,
    pub bid_size: Decimal,
    pub ask_size: Decimal,
    pub last: Decimal,
    #[serde_as(as = "TimestampSecondsWithFrac<f64>")]
    pub time: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FillInfo {
    pub id: Id,
    pub market: Option<Symbol>,
    pub future: Option<Symbol>,
    pub base_currency: Option<Coin>,
    pub quote_currency: Option<Coin>,
    pub r#type: String, // e.g. "order"
    pub side: Side,
    pub price: Decimal,
    pub size: Decimal,
    pub order_id: Option<Id>,
    pub trade_id: Option<Id>,
    pub time: DateTime<Utc>,
    pub fee: Decimal,
    pub fee_rate: Decimal,
    pub fee_currency: Coin,
    pub liquidity: Liquidity,
}

/// Order book data received from FTX which is used for initializing and updating
/// the OrderBook struct
#[serde_as]
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderbookInfo {
    pub action: WsOrderbookAction,
    pub bids: Vec<(Decimal, Decimal)>,
    pub asks: Vec<(Decimal, Decimal)>,
    pub checksum: Checksum,
    #[serde_as(as = "TimestampSecondsWithFrac<f64>")]
    pub time: DateTime<Utc>, // API returns 1621740952.5079553
}

type Checksum = u32;

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WsOrderbookAction {
    /// Initial snapshot of the orderbook
    Partial,
    /// Updates to the orderbook
    Update,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Liquidity {
    Maker,
    Taker,
}