use std::{sync::Arc, time::Duration};

use connector_ftx::ws::{
    EventHandler, FtxWsClient, WsChannel, WsDataEvent, WsResponseData
};
use rust_extensions::Logger;

pub struct OrderBookHandler {}

impl OrderBookHandler {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl EventHandler for OrderBookHandler {
    async fn on_data(&self, event: WsDataEvent) {
        if let WsResponseData::OrderbookData(orderbook_data) = event.data {
            println!("Recieved orderbook {}:", event.market.unwrap());
            println!("{:?}", orderbook_data);
            println!("-------------------------------");
        }
    }
}

pub struct ConsoleLogger {}

impl Logger for ConsoleLogger {
    fn write_info(&self, _process: String, _message: String, _ctx: Option<std::collections::HashMap<String, String>>) {
        
    }

    fn write_warning(&self, _process: String, _message: String, _ctx: Option<std::collections::HashMap<String, String>>) {
    }

    fn write_error(&self, _process: String,_messagee: String, _ctx: Option<std::collections::HashMap<String, String>>) {
    }

    fn write_fatal_error(
        &self,
        _process: String,
        _message: String,
        _ctx: Option<std::collections::HashMap<String, String>>,
    ) {
    }
}

#[tokio::main]
async fn main() {
    let channels = vec![
        WsChannel::Orderbook("BTC/USD".to_owned()),
        WsChannel::Orderbook("ETH/USD".to_owned()),
    ];
    let event_handler = Arc::new(OrderBookHandler {});
    let ftx_ws = FtxWsClient::new(
        event_handler,
        Arc::new(ConsoleLogger{}),
        channels,
    );

    FtxWsClient::start(Arc::new(ftx_ws));


    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}
