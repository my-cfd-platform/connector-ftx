use std::{sync::Arc, time::Duration};

use connector_ftx::ws::{EventHandler, FtxWsClient, WsChannel, WsResponseData, WsDataEvent};

pub struct OrderBookHandler {}

impl OrderBookHandler {
    pub fn new() -> Self {
        Self {}
    }
}

impl EventHandler for OrderBookHandler {
    fn on_data(&self, event: WsDataEvent) {
        if let WsResponseData::OrderbookData(orderbook_data) = event.data {
            println!("Recieved orderbook {}:", event.market.unwrap());
            println!("{:?}", orderbook_data);
            println!("-------------------------------");
        }
    }
}

#[tokio::main]
async fn main() {
    let mut ftx_ws = FtxWsClient::new(Arc::new(OrderBookHandler::new()), None);
    ftx_ws.connect().await.unwrap();
    ftx_ws
        .subscribe(&[
            WsChannel::Orderbook("BTC/USD".to_owned()),
            WsChannel::Orderbook("ETH/USD".to_owned())
            ])
        .await
        .unwrap();
    ftx_ws.start();

    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
