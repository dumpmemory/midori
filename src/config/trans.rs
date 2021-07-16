use serde::{Serialize, Deserialize};

use super::WithTransport;
use crate::transport::ws;
use crate::transport::{AsyncConnect, AsyncAccept};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "proto", rename_all = "lowercase")]
pub enum TransportConfig {
    Plain,
    WS(WebSocketConfig),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketConfig {
    pub path: String,
}

impl<L, C> WithTransport<L, C> for WebSocketConfig
where
    L: AsyncAccept,
    C: AsyncConnect,
{
    type Acceptor = ws::Acceptor<L>;
    type Connector = ws::Connector<C>;

    fn apply_to_lis(&self, lis: L) -> Self::Acceptor {
        ws::Acceptor::new(lis, self.path.clone())
    }
    fn apply_to_conn(&self, conn: C) -> Self::Connector {
        ws::Connector::new(conn, self.path.clone())
    }
}
