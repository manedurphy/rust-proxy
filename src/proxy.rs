use bytes::{BufMut, BytesMut};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
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
    listen: u32,
    forward: Forward,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    path: String,
    proxy_pass: String,
}

pub struct Proxy {
    cfg: Config,
    tracker: Arc<AsyncMutex<usize>>,
}

impl Proxy {
    pub fn new(cfg: Config) -> Self {
        Proxy {
            cfg,
            tracker: Arc::new(AsyncMutex::new(0)),
        }
    }

    pub async fn start(self: Arc<Self>) -> io::Result<()> {
        tokio::spawn(async move {
            let s = Arc::clone(&self);
            for server in s.cfg.servers.as_slice() {
                let forward_type = server.forward.clone();
                let addr = format!("0.0.0.0:{}", server.listen);

                match forward_type {
                    Forward::LoadBalancer(loc) => {
                        if let Ok(listener) = TcpListener::bind(&addr).await {
                            let s = Arc::clone(&s);
                            info!("listening on {}", addr);

                            tokio::spawn(async move {
                                loop {
                                    let (mut socket, _) = listener.accept().await.unwrap();

                                    info!("incoming request");
                                    let req = s.read_request(&mut socket).await.unwrap();
                                    let mut index = s.tracker.lock().await;
                                    let backend = s.cfg.upstream.get(&loc).unwrap();
                                    let upstream_server = &backend[*index];
                                    debug!("request: {:#?}", req);

                                    *index += 1;
                                    if *index > backend.len() - 1 {
                                        *index = 0;
                                    }
                                    drop(index);

                                    let path = req.path.clone();
                                    let resp = s
                                        .forward_request(
                                            req,
                                            format!("http://{}{}", upstream_server, path).as_str(),
                                        )
                                        .await;

                                    s.write_to_socket(socket, resp).await;
                                }
                            });
                        }
                    }
                    Forward::Locations(locations) => {
                        let listener = TcpListener::bind(&addr).await.unwrap();
                        let s = Arc::clone(&s);
                        info!("listening on {}", addr);

                        tokio::spawn(async move {
                            loop {
                                let (mut socket, _) = listener.accept().await.unwrap();
                                let locations = locations.clone();

                                info!("incoming request");
                                let s = Arc::clone(&s);
                                tokio::spawn(async move {
                                    let req = s.read_request(&mut socket).await.unwrap();
                                    let mut resp = String::new();
                                    debug!("request: {:#?}", req);

                                    for location in locations {
                                        if location.path == req.path {
                                            resp =
                                                s.forward_request(req, &location.proxy_pass).await;
                                            break;
                                        }
                                    }

                                    s.write_to_socket(socket, resp).await;
                                });
                            }
                        });
                    }
                }
            }
        });
        Ok(())
    }

    async fn read_request(&self, socket: &mut TcpStream) -> Option<HTTPRequest> {
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

    async fn forward_request(&self, request: HTTPRequest, proxy_pass: &str) -> String {
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

    async fn write_to_socket(&self, mut socket: TcpStream, resp: String) {
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

/*
    // Reference for loadbalancer
    pub async fn start(self: Arc<Self>) -> io::Result<()> {
        let max_length = self.cfg.upstream.len() - 1;
        let listener = TcpListener::bind(&self.cfg.addr).await?;
        info!("listening on {}", self.cfg.addr);

        loop {
            let (mut socket, _) = listener.accept().await?;

            info!("incoming request");
            let s = Arc::clone(&self);
            tokio::spawn(async move {
                let mut index = s.tracker.lock().await;
                let upstream = &s.cfg.upstream[*index];
                *index += 1;
                if *index > max_length {
                    *index = 0;
                }
                drop(index);

                let request = s.read_request(&mut socket).await.unwrap();
                let mut response = String::new();
                debug!("request: {:#?}", request);

                if s.cfg.locations.contains(&request.path) {
                    response = s.forward_request(request, upstream).await;
                }

                socket.write(response.as_bytes()).await.unwrap();
                socket.flush().await.unwrap();
            });
        }
    }


        async fn forward_request(&self, request: HTTPRequest, upstream: &str) -> String {
        let client = reqwest::Client::new();

        info!("forwarding request to upstream: {}", upstream);
        let resp = client
            .get(format!("http://{}{}", upstream, request.path))
            .headers(request.headers)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();

        format!(
            "{}\r\nContent-Length: {}\r\n\r\n{}",
            "HTTP/1.1 200 OK",
            resp.len(),
            resp
        )
    }



     if loc.starts_with("http://") {
                                tokio::spawn(async move {
                                    loop {
                                        if let Ok((mut socket, _)) = listener.accept().await {
                                            info!("incoming request");
                                            if let Some(req) = s.read_request(&mut socket).await {
                                                debug!("request: {:#?}", req);

                                                let response = s.forward_request(req, &loc).await;
                                                // let write_resp = socket.write(response.as_bytes()).await.unwrap();
                                                match socket.write(response.as_bytes()).await {
                                                    Ok(_) => {
                                                        info!(
                                                            "data successfully written to socket"
                                                        );
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
                                                        error!(
                                                            "could not flush output stream: {}",
                                                            err.to_string()
                                                        );
                                                    }
                                                };
                                            }
                                        }
                                    }
                                });
                                continue;
                            }
*/
