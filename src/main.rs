use clap::{App, Arg};
use log::{self, debug, info};
use proxy::{Config as ProxyConfig, Proxy};
use serde::{Deserialize, Serialize};
use static_file_server::{Config as SFSConfig, StaticFileServer};
use std::sync::Arc;
use std::thread;
use tokio::fs;
use tokio::net::TcpListener;

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

    let matches = App::new("Rust Proxy")
        .version("1.0.0")
        .author("Dane M. Murphy")
        .arg(
            Arg::new("config")
                .short('c')
                .long("config")
                .value_name("FILE")
                .help("Sets the path for the config file")
                .takes_value(true),
        )
        .get_matches();

    let config = parse_config(matches.value_of("config").unwrap_or("configs/proxy.json")).await;
    debug!("config: {:#?}", config);

    if config.static_files.enabled {
        let file_server = StaticFileServer::new(config.static_files.path);
        file_server.start();
    }

    let http_proxy = Proxy::new(config.proxy);
    for server in &http_proxy.cfg.servers {
        let p = Arc::clone(&http_proxy);
        let addr = format!("0.0.0.0:{}", server.listen);
        let forward = server.forward.clone();

        match forward {
            proxy::Forward::LoadBalancer(lb) => {
                if let Ok(listener) = TcpListener::bind(&addr).await {
                    info!("listening on {}", addr);
                    tokio::spawn(async move {
                        loop {
                            let (mut socket, _) = listener.accept().await.unwrap();

                            info!("incoming request");
                            let req = p.read_request(&mut socket).await.unwrap();
                            let mut index = p.tracker.lock().await;
                            let backend = p.cfg.upstream.get(&lb).unwrap();
                            let upstream_server = &backend[*index];
                            debug!("request: {:#?}", req);

                            *index += 1;
                            if *index > backend.len() - 1 {
                                *index = 0;
                            }
                            drop(index);

                            let path = req.path.clone();
                            let proxy_pass = format!("http://{}{}", upstream_server, path);
                            let resp = p.forward_request(req, proxy_pass.as_str()).await;

                            p.write_to_socket(socket, resp).await;
                        }
                    });
                }
            }
            proxy::Forward::Locations(locations) => {
                if let Ok(listener) = TcpListener::bind(&addr).await {
                    info!("listening on {}", addr);
                    tokio::spawn(async move {
                        loop {
                            let (mut socket, _) = listener.accept().await.unwrap();

                            info!("incoming request");
                            let req = p.read_request(&mut socket).await.unwrap();

                            // TODO: make a real default reponse
                            let mut resp = String::new();
                            debug!("request: {:#?}", req);

                            for location in &locations {
                                if location.path == req.path {
                                    resp = p.forward_request(req, &location.proxy_pass).await;
                                    break;
                                }
                            }

                            p.write_to_socket(socket, resp).await;
                        }
                    });
                }
            }
        }
    }
    thread::park();
}

async fn parse_config(file: &str) -> Config {
    let proxy_config = fs::read(file).await.unwrap();
    let config_str = String::from_utf8(proxy_config).unwrap();

    let tc: Config = serde_json::from_str(config_str.as_str()).unwrap();
    tc
}
