use std::io;
use tokio::task::JoinHandle;

use super::common;
use super::transport;
use crate::config::{EpHalfConfig, NetConfig, TransportConfig, TLSConfig};
use crate::transport::AsyncConnect;
use crate::transport::plain::{self, PlainListener};
use crate::transport::udp;
use crate::transport::quic;

pub fn new_plain_conn(addr: &str, net: &NetConfig) -> plain::Connector {
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use crate::utils::CommonAddr;
    match net {
        NetConfig::TCP => {
            let (sockaddr, _) = common::parse_socket_addr(addr, true).unwrap();
            plain::Connector::new(sockaddr)
        }
        #[cfg(unix)]
        NetConfig::UDS => {
            let path = CommonAddr::UnixSocketPath(PathBuf::from(addr));
            plain::Connector::new(path)
        }
        _ => unreachable!(),
    }
}

pub fn new_plain_lis(addr: &str, net: &NetConfig) -> plain::Acceptor {
    #[cfg(unix)]
    use std::path::PathBuf;
    #[cfg(unix)]
    use crate::utils::CommonAddr;
    match net {
        NetConfig::TCP => {
            let (sockaddr, _) = common::parse_socket_addr(addr, false).unwrap();
            let lis = PlainListener::bind(&sockaddr).unwrap();
            plain::Acceptor::new(lis, sockaddr)
        }
        #[cfg(unix)]
        NetConfig::UDS => {
            let path = CommonAddr::UnixSocketPath(PathBuf::from(addr));
            let lis = PlainListener::bind(&path).unwrap();
            plain::Acceptor::new(lis, path)
        }
        _ => unreachable!(),
    }
}

// ===== UDP =====
pub fn new_udp_conn(addr: &str, _: &NetConfig) -> udp::Connector {
    let (sockaddr, _) = common::parse_socket_addr(addr, true).unwrap();
    udp::Connector::new(sockaddr)
}

pub fn new_udp_lis(addr: &str, _: &NetConfig) -> udp::Acceptor {
    let (sockaddr, _) = common::parse_socket_addr(addr, false).unwrap();
    udp::Acceptor::new(sockaddr)
}

// ===== QUIC =====
use std::sync::Arc;
use quinn::{Endpoint, ClientConfig, ServerConfig};
use crate::utils;

pub fn new_quic_conn(
    addr: &str,
    _: &NetConfig,
    trans: &TransportConfig,
    tlsc: &TLSConfig,
) -> quic::Connector {
    // check transport
    let trans = match trans {
        TransportConfig::QUIC(x) => x,
        _ => unreachable!(),
    };
    // check tls
    let tlsc = match tlsc {
        TLSConfig::Client(x) => x,
        _ => unreachable!(),
    };

    let (sockaddr, is_ipv6) = common::parse_socket_addr(addr, true).unwrap();
    let mut client_tls = tlsc.to_tls();
    let sni = tlsc.set_sni(&mut client_tls, &sockaddr);

    let mut client_config = ClientConfig::default();
    // default:
    // set ciphersuits = QUIC_CIPHER_SUITES
    // set versions = TLSv1_3
    // set enable_early_data = true
    client_tls.ciphersuites = client_config.crypto.ciphersuites.clone();
    client_tls.versions = client_config.crypto.versions.clone();
    client_tls.enable_early_data = client_config.crypto.enable_early_data;
    client_config.crypto = Arc::new(client_tls);

    let bind_addr = if is_ipv6 {
        utils::empty_sockaddr_v6()
    } else {
        utils::empty_sockaddr_v4()
    };

    let mut builder = Endpoint::builder();
    builder.default_client_config(client_config);
    let (ep, _) = builder.bind(&bind_addr).unwrap();
    quic::Connector::new(ep, sockaddr, sni, trans.mux)
}

pub fn new_quic_raw_lis(
    addr: &str,
    _: &NetConfig,
    trans: &TransportConfig,
    tlsc: &TLSConfig,
) -> quic::RawAcceptor {
    // check transport
    match trans {
        TransportConfig::QUIC(x) => x,
        _ => unreachable!(),
    };
    // check tls
    let tlsc = match tlsc {
        TLSConfig::Server(x) => x,
        _ => unreachable!(),
    };

    let (sockaddr, _) = common::parse_socket_addr(addr, false).unwrap();
    let bind_addr = match sockaddr {
        utils::CommonAddr::SocketAddr(ref x) => x,
        _ => unreachable!(),
    };

    let mut server_tls = tlsc.to_tls();
    let mut server_config = ServerConfig::default();
    // default:
    // set ciphersuits = QUIC_CIPHER_SUITES
    // set versions = TLSv1_3
    // set max_early_data_size = u32::max_value()
    server_tls.ciphersuites = server_config.crypto.ciphersuites.clone();
    server_tls.versions = server_config.crypto.versions.clone();
    server_tls.max_early_data_size = server_config.crypto.max_early_data_size;
    server_config.crypto = Arc::new(server_tls);

    let mut builder = Endpoint::builder();
    builder.listen(server_config);
    let (_, incoming) = builder.bind(bind_addr).expect("failed to bind");
    quic::RawAcceptor::new(incoming, sockaddr)
}

pub fn spawn_with_net(
    workers: &mut Vec<JoinHandle<io::Result<()>>>,
    listen: &EpHalfConfig,
    remote: &EpHalfConfig,
) {
    match listen.net {
        NetConfig::TCP | NetConfig::UDS => {
            let lis = new_plain_lis(&listen.addr, &listen.net);
            match remote.net {
                NetConfig::TCP | NetConfig::UDS => {
                    let conn = new_plain_conn(&remote.addr, &remote.net);
                    transport::spawn_with_trans(
                        workers, listen, remote, lis, conn,
                    )
                }
                NetConfig::UDP => {
                    let conn = new_udp_conn(&remote.addr, &remote.net);
                    transport::spawn_with_trans(
                        workers, listen, remote, lis, conn,
                    )
                }
            }
        }
        NetConfig::UDP => {
            let lis = new_udp_lis(&listen.addr, &listen.net);
            match remote.net {
                NetConfig::TCP | NetConfig::UDS => {
                    let conn = new_plain_conn(&remote.addr, &remote.net);
                    transport::spawn_with_trans(
                        workers, listen, remote, lis, conn,
                    )
                }
                NetConfig::UDP => {
                    let conn = new_udp_conn(&remote.addr, &remote.net);
                    transport::spawn_with_trans(
                        workers, listen, remote, lis, conn,
                    )
                }
            }
        }
    }
}
