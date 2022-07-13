use bytes::{Bytes, BytesMut};
use log::{debug, info};
use reqwest::header::{HeaderMap, HeaderName};
use std::collections::HashMap;
use std::str::FromStr;

#[derive(Debug)]
pub enum HTTPMethod {
    Get,
    Post,
    Put,
    Delete,
}

#[derive(Debug)]
pub struct HTTPRequest {
    pub method: HTTPMethod,
    pub path: String,
    pub protocol: String,
    pub headers: HeaderMap,
    pub body: Bytes,
}

pub struct HTTPResponse {
    pub status_code: u32,
    pub headers: HashMap<String, String>,
    pub body: Bytes,
}

impl HTTPRequest {
    pub fn new() -> Self {
        HTTPRequest {
            method: HTTPMethod::Get,
            path: "/".to_string(),
            protocol: "HTTP/1.1".to_string(),
            headers: HeaderMap::new(),
            body: Bytes::new(),
        }
    }

    pub fn parse(&mut self, buffer: &BytesMut) {
        info!("parsing data from socket");

        let body = Bytes::new();
        let method = HTTPMethod::Get;
        let protocol = "HTTP/1.1".to_string();
        let buffer_str = String::from_utf8(buffer.to_vec()).unwrap();
        let p: Vec<&str> = buffer_str.split(' ').collect();
        let path = p.get(1).unwrap().to_string();
        let mut headers = HeaderMap::new();

        for line in buffer_str.split("\r\n").into_iter().skip(1) {
            debug!("line: {}", line);

            if let Some(splitter) = line.find(": ") {
                let header_key = String::from(&line[..splitter]);
                let header_value = String::from(&line[splitter + 2..]);

                headers.insert(
                    HeaderName::from_str(header_key.as_str()).unwrap(),
                    header_value.as_str().parse().unwrap(),
                );
            }
        }

        self.method = method;
        self.path = path;
        self.protocol = protocol;
        self.headers = headers;
        self.body = body;
    }
}
