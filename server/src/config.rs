use std::{env, net::SocketAddr};

use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_addr: SocketAddr,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("invalid value for {var}: {source}")]
    InvalidValue {
        var: &'static str,
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        let server_addr = parse_env("SERVER_ADDR", "0.0.0.0:3000")?;
        Ok(Self { server_addr })
    }
}

fn parse_env<T>(var: &'static str, default: &str) -> Result<T, ConfigError>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    let raw = env::var(var).unwrap_or_else(|_| default.to_owned());
    raw.parse::<T>().map_err(|e| ConfigError::InvalidValue {
        var,
        source: Box::new(e),
    })
}
