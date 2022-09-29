use super::{models::*, WsError};

#[async_trait::async_trait]
pub trait EventHandler {
    async fn on_data(&self, event: WsDataEvent);

    fn on_connect(&self) {
        println!("Connected to ws");
    }

    fn on_auth(&self) {
        println!("Authentificated to ws");
    }

    fn on_subscribed(&self, channel: &str, market: &str) {
        println!("Subscribed to {} {}", channel, market);
    }

    fn on_error(&self, message: WsError) {
        println!("Error on ws {}", message);
    }
}

pub struct WsDataEvent {
    pub data: WsResponseData,
    pub market: Option<Symbol>,
}

impl WsDataEvent {
    pub fn new(resp: WsResponse) -> Self {
        Self {
            data: resp.data.unwrap(),
            market: resp.market,
        }
    }
}
