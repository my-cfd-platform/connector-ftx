use super::{models::*};

#[async_trait::async_trait]
pub trait EventHandler {
    async fn on_data(&self, event: WsDataEvent);
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
