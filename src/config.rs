use std::fs::File;
use std::io::Read;

use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};

lazy_static! {
    pub static ref CONF: Config = {
        let mut file = File::open("/tmp/config.toml").expect("invaid file path");
        let mut content = String::new();
        file.read_to_string(&mut content)
            .expect("read config failed");
        toml::from_str::<Config>(content.as_str()).unwrap()
    };
}
#[derive(Serialize, Deserialize)]
pub struct Config {
    pub sources: Vec<Source>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Source {
    pub ip: String,
    pub port: usize,
    pub ssh_user: Option<String>,
    pub ssh_password: Option<String>,
    pub local_port: Option<usize>,
    pub db: String,
}
