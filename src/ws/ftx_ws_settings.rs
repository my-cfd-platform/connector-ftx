use my_web_socket_client::WsClientSettings;

pub struct FtxWsSetting {}

impl FtxWsSetting {
    pub fn new() -> Self {
        Self{}
    }
}

#[async_trait::async_trait]
impl WsClientSettings for FtxWsSetting {
    async fn get_url(&self) -> String {
        return "wss://ftx.com/ws".to_string();
    }
}