use std::{env, net::IpAddr, str::FromStr, time::Duration};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub host: IpAddr,
    pub port: u16,
    pub max_connections: u32,
    pub database_timeout: Duration,
    pub request_timeout: Duration,
    pub shutdown_timeout: Duration,
    pub event_buffer: usize,
}

impl Config {
    pub fn from_env() -> Result<Self, String> {
        let database_url =
            env::var("DATABASE_URL").map_err(|_| "DATABASE_URL must be set".to_owned())?;
        Ok(Self {
            database_url,
            host: parse("APP_HOST", "0.0.0.0")?,
            port: parse("APP_PORT", "8080")?,
            max_connections: parse("DATABASE_MAX_CONNECTIONS", "20")?,
            database_timeout: Duration::from_secs(parse("DATABASE_TIMEOUT_SECONDS", "5")?),
            request_timeout: Duration::from_secs(parse("REQUEST_TIMEOUT_SECONDS", "10")?),
            shutdown_timeout: Duration::from_secs(parse("SHUTDOWN_TIMEOUT_SECONDS", "30")?),
            event_buffer: parse("EVENT_BUFFER", "1024")?,
        })
    }
}

fn parse<T: FromStr>(name: &str, default: &str) -> Result<T, String>
where
    T::Err: std::fmt::Display,
{
    env::var(name)
        .unwrap_or_else(|_| default.to_owned())
        .parse()
        .map_err(|err| format!("Invalid {name}: {err}"))
}
