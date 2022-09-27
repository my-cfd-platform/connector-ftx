pub use crate::common::{Coin, Id, MarketType, OrderInfo, Side, Symbol, Trade};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, TimestampSecondsWithFrac};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Channel {
    Orderbook(Symbol),
    Trades(Symbol),
    Ticker(Symbol),
    Fills,
    Orders,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub market: Option<Symbol>,
    pub r#type: Type,
    pub data: Option<ResponseData>,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Type {
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
pub enum ResponseData {
    Ticker(Ticker),
    Trades(Vec<Trade>),
    OrderbookData(OrderbookData),
    Fill(Fill),
    Order(OrderInfo),
}

/// Represents the data we return to the user
#[derive(Clone, Debug, Serialize)]
pub enum Data {
    Ticker(Ticker),
    Trade(Trade),
    OrderbookData(OrderbookData),
    Fill(Fill),
    Order(OrderInfo),
}

#[serde_as]
#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
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
pub struct Fill {
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
pub struct OrderbookData {
    pub action: OrderbookAction,
    // Note that bids and asks are returned in 'best' order,
    // i.e. highest to lowest bids, lowest to highest asks
    pub bids: Vec<(Decimal, Decimal)>,
    pub asks: Vec<(Decimal, Decimal)>,
    pub checksum: Checksum,
    #[serde_as(as = "TimestampSecondsWithFrac<f64>")]
    pub time: DateTime<Utc>, // API returns 1621740952.5079553
}

type Checksum = u32;

#[derive(Copy, Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum OrderbookAction {
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