use crate::channels::traits::{self, Channel};
use async_trait::async_trait;
use tokio::sync::mpsc::Sender;

pub struct IMessageChannel {
    _allowed_contacts: Vec<String>,
}

impl IMessageChannel {
    pub fn new(allowed_contacts: Vec<String>) -> Self {
        Self { _allowed_contacts: allowed_contacts }
    }
}

#[async_trait]
impl Channel for IMessageChannel {
    fn name(&self) -> &str { "imessage" }
    async fn send(&self, _: &str, _: &str) -> anyhow::Result<()> {
        anyhow::bail!("iMessage not supported in SaaS environment")
    }
    async fn listen(&self, _: Sender<traits::ChannelMessage>) -> anyhow::Result<()> {
        std::future::pending::<()>().await;
        Ok(())
    }
}
