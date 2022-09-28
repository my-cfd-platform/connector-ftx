use serde::Serialize;

use super::models::*;

pub trait EventHandler {
    fn on_event(&self, event: EventData, symbol: Option<Symbol>);
}

#[derive(Clone, Debug, Serialize)]
pub enum EventData {
    Ticker(TickerInfo),
    Trade(TradeInfo),
    OrderbookData(OrderbookInfo),
    Fill(FillInfo),
    Order(OrderInfo),
}