#[derive(Debug, serde::Deserialize)]
pub struct Config {
    pub mqtt_broker_uri: std::net::SocketAddr,
    pub mqtt_broker_port: u16,
    pub session_expiry_interval: u16,

    pub message_timeout_millis: u16,

    pub mappings: Vec<Mapping>,
}

#[derive(Debug, serde::Deserialize)]
pub struct Mapping {
    pub topic: String,
}
