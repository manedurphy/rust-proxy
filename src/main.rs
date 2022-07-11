use log::{self, debug};
use proxy::{Config as ProxyConfig, Proxy};
use serde::{Deserialize, Serialize};
use static_file_server::{Config as SFSConfig, StaticFileServer};
use std::sync::Arc;
use std::thread;
use tokio::fs;

mod http;
mod proxy;
mod static_file_server;

#[derive(Debug, Serialize, Deserialize)]
struct Config {
    static_files: SFSConfig,
    proxy: ProxyConfig,
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let config = parse_config().await;
    debug!("config: {:#?}", config);

    if config.static_files.enabled {
        let file_server = StaticFileServer::new(config.static_files.path);
        file_server.start();
    }

    let proxy = Proxy::new(config.proxy);
    if let Ok(()) = Proxy::start(Arc::new(proxy)).await {
        thread::park();
    };
}

async fn parse_config() -> Config {
    let proxy_config = fs::read("configs/proxy.json").await.unwrap();
    let config_str = String::from_utf8(proxy_config).unwrap();

    println!("{}", config_str);

    let tc: Config = serde_json::from_str(config_str.as_str()).unwrap();
    tc
}
