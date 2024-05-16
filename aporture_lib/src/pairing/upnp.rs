use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use igd::aio::tokio::Tokio;
use igd::aio::Gateway as IgdGateway;
use igd::{PortMappingProtocol, SearchOptions};
use thiserror::Error;

#[derive(Debug)]
pub struct Gateway {
    igd: IgdGateway<Tokio>,
    ip: IpAddr,
    port: Option<u16>,
}

impl Gateway {
    pub async fn new() -> Result<Self, Error> {
        let search_options = SearchOptions {
            timeout: Some(Duration::from_secs(2)),
            ..Default::default()
        };

        let ip = local_ip_address::local_ip()?;

        let igd = igd::aio::tokio::search_gateway(search_options).await?;

        Ok(Self {
            igd,
            ip,
            port: None,
        })
    }

    pub async fn open_port(&mut self, port: u16) -> Result<SocketAddr, Error> {
        const PORT_DESCRIPTION: &str = "aporture";

        if self.port.is_some() {
            // NOTE: Ignore error because port might already be closed
            let _ = self.close_port().await;
        }

        let local_address = (self.ip, port).into();

        let external_address = self
            .igd
            .get_any_address(
                PortMappingProtocol::TCP,
                local_address,
                3600,
                PORT_DESCRIPTION,
            )
            .await;

        let external_address = match external_address {
            Err(igd::AddAnyPortError::OnlyPermanentLeasesSupported) => {
                log::warn!("Router does not support temporary upnp, trying permanent leasing");

                self.igd
                    .get_any_address(PortMappingProtocol::TCP, local_address, 0, PORT_DESCRIPTION)
                    .await
            }
            a => a,
        }?;

        self.port = Some(external_address.port());

        Ok(external_address)
    }

    pub async fn close_port(&mut self) -> Result<(), Error> {
        if let Some(port) = self.port.take() {
            self.igd
                .remove_port(igd::PortMappingProtocol::TCP, port)
                .await?;
        };

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Could not find local ip address")]
    LocalIpNotFound,
    #[error("Could not find upnp enabled gateway")]
    GatewayNotFound,
    #[error("Could not operate upnp gateway to open port")]
    OpenPort,
    #[error("Last port was already closed or never opened")]
    ClosePort,
    #[error("Could not perform operation on gateway")]
    UPnP,
}

impl From<local_ip_address::Error> for Error {
    fn from(_: local_ip_address::Error) -> Self {
        Self::LocalIpNotFound
    }
}

impl From<igd::SearchError> for Error {
    fn from(_: igd::SearchError) -> Self {
        Self::GatewayNotFound
    }
}

impl From<igd::AddAnyPortError> for Error {
    fn from(_: igd::AddAnyPortError) -> Self {
        Self::OpenPort
    }
}

impl From<igd::RemovePortError> for Error {
    fn from(value: igd::RemovePortError) -> Self {
        match value {
            igd::RemovePortError::NoSuchPortMapping => Self::ClosePort,
            _ => Self::UPnP,
        }
    }
}
