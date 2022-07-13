use bytes::{BufMut, BytesMut};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex as AsyncMutex;

use crate::http::HTTPRequest;

const MAX_CHUNK_SIZE: usize = 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "backend")]
pub enum Forward {
    LoadBalancer(String),
    Locations(Vec<Location>),
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub upstream: HashMap<String, Vec<String>>,
    pub servers: Vec<Server>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub listen: u32,
    pub forward: Forward,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub path: String,
    pub proxy_pass: String,
}

pub struct Proxy {
    pub cfg: Config,
    pub tracker: Arc<AsyncMutex<usize>>,
}

impl Proxy {
    pub fn new(cfg: Config) -> Arc<Self> {
        Arc::new(Self {
            cfg,
            tracker: Arc::new(AsyncMutex::new(0)),
        })
    }

    pub async fn read_request(&self, socket: &mut TcpStream) -> Option<HTTPRequest> {
        let mut buffer = BytesMut::new();
        let mut request = HTTPRequest::new();

        loop {
            let mut chunk = vec![0; MAX_CHUNK_SIZE];
            match socket.read(&mut chunk).await {
                Ok(0) => {
                    break;
                }
                Ok(c_size) => {
                    buffer.put(&chunk[..c_size]);
                    debug!("chunk size: {}", c_size);

                    if c_size < MAX_CHUNK_SIZE {
                        break;
                    }
                }
                Err(err) => {
                    error!("could not read from socket: {}", err);
                    return None;
                }
            }
        }

        request.parse(&buffer);
        Some(request)
    }

    pub async fn forward_request(&self, request: HTTPRequest, proxy_pass: &str) -> String {
        let client = reqwest::Client::new();

        info!("forwarding request to upstream: {}", proxy_pass);
        if let Ok(resp) = client.get(proxy_pass).headers(request.headers).send().await {
            let status_line = resp.status().to_string();
            if let Ok(test_resp) = resp.text().await {
                return format!(
                    "HTTP/1.1 {}\r\nContent-Length: {}\r\n\r\n{}",
                    status_line,
                    test_resp.len(),
                    test_resp
                );
            }
        }

        String::from("internal server error")
    }

    pub async fn write_to_socket(&self, mut socket: TcpStream, resp: String) {
        match socket.write(resp.as_bytes()).await {
            Ok(_) => {
                info!("data successfully written to socket");
            }
            Err(err) => {
                error!("could not write response to socket: {}", err.to_string());
            }
        }

        match socket.flush().await {
            Ok(_) => {
                info!("data successfully flushed");
            }
            Err(err) => {
                error!("could not flush output stream: {}", err.to_string());
            }
        };
    }
}
