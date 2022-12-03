use area_net::network::{controller::NetworkController, Network};
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let opt = Opt::parse();

    let config: Config = match opt.try_into() {
        Ok(config) => config,
        Err(err) => {
            log::error!("Configuration Error: {err}");
            std::process::exit(1);
        }
    };

    log::info!("config: {}", serde_json::to_string(&config).unwrap());

    if let Err(err) = run(config).await {
        log::error!("Error: {err}");
        std::process::exit(1);
    } else {
        Ok(())
    }
}

async fn run(config: Config) -> Result<(), Error> {
    let mut controller =
        NetworkController::new(config.label, config.network.controller).map_err(|err| {
            Error::Controller {
                source: err,
                detail: "Could not create network controller".to_owned(),
            }
        })?;

    controller
        .initialize()
        .await
        .map_err(|err| Error::Controller {
            source: err,
            detail: "Could not initialize network controller".to_owned(),
        })?;

    controller.run().await.map_err(|err| Error::Controller {
        source: err,
        detail: "An error occured while running the network controller".to_owned(),
    })
}

/// Peer 2 Peer Network Controller
///
/// This program starts a node in a peer 2 peer network.
/// This node will listen to incoming connection requests,
/// and also try to connect to peer nodes depending on the
/// configuration.
#[derive(Parser)]
#[command(author, version, about, long_about)]
struct Opt {
    /// Root configuration file.
    #[arg(value_parser = clap::value_parser!(PathBuf), short = 'c', long = "config-dir")]
    pub config_dir: PathBuf,

    /// Configuration overrides
    #[arg(short = 's', long = "setting")]
    pub settings: Vec<String>,

    /// set the log level
    #[clap(short = 'p', long = "profile")]
    pub profile: Option<String>,
}

/// Top level error type.
#[derive(Debug)]
pub enum Error {
    /// Node Error
    Controller {
        /// Source
        source: area_net::network::controller::Error,
        /// Error detail
        detail: String,
    },

    /// Configuration Error
    Configuration {
        /// Source
        source: area_net::config::Error,
    },

    /// Configuration Error
    ConfigurationDeserialization {
        /// Source
        source: config::ConfigError,
    },
}

impl From<area_net::config::Error> for Error {
    fn from(err: area_net::config::Error) -> Self {
        Error::Configuration { source: err }
    }
}

impl From<config::ConfigError> for Error {
    fn from(err: config::ConfigError) -> Self {
        Error::ConfigurationDeserialization { source: err }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Controller { source, detail } => {
                write!(f, "Network Controller Error: {} => {}", source, detail)
            }
            Error::Configuration { source } => {
                write!(f, "Configuration Error: {}", source)
            }
            Error::ConfigurationDeserialization { source } => {
                write!(f, "Configuration Deserialization Error: {}", source)
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub label: String,
    pub network: Network,
}

impl TryInto<Config> for Opt {
    type Error = Error;

    fn try_into(self) -> Result<Config, Self::Error> {
        let config = area_net::config::merge_configuration(
            self.config_dir.as_ref(),
            &["network"],
            self.profile.as_deref(),
            self.settings.clone(),
        )?
        .try_deserialize()?;
        Ok(config)
    }
}
