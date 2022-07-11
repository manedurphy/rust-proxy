use rocket_contrib::serve::StaticFiles;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub enabled: bool,
    pub path: String,
}

pub struct StaticFileServer {
    files: String,
}

impl StaticFileServer {
    pub fn new(files: String) -> Self {
        StaticFileServer { files }
    }

    pub fn start(self) {
        tokio::spawn(async move {
            rocket::ignite()
                .mount("/", StaticFiles::from(self.files))
                .launch();
        });
    }
}
