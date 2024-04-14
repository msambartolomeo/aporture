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
    pub async fn new() -> Result<Self, GatewayError> {
        let search_options = SearchOptions {
            timeout: Some(Duration::from_secs(2)),
            ..Default::default()
        };

        let ip = local_ip_address::local_ip().map_err(|_| GatewayError::LocalIpNotFound)?;

        let igd = igd::aio::tokio::search_gateway(search_options)
            .await
            .map_err(|_| GatewayError::GatewayNotFound)?;

        Ok(Self {
            igd,
            ip,
            port: None,
        })
    }

    pub async fn open_port(&mut self, port: u16) -> Result<SocketAddr, OpenPortError> {
        const PORT_DESCRIPTION: &str = "aporture";

        if self.port.is_some() {
            self.close_port().await.map_err(|_| OpenPortError)?;
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
                    .map_err(|_| OpenPortError)
            }
            a => a.map_err(|_| OpenPortError),
        }?;

        self.port = Some(external_address.port());

        Ok(external_address)
    }

    pub async fn close_port(&mut self) -> Result<(), ClosePortError> {
        if let Some(port) = self.port.take() {
            self.igd
                .remove_port(igd::PortMappingProtocol::TCP, port)
                .await
                .map_err(|e| match e {
                    igd::RemovePortError::NoSuchPortMapping => ClosePortError::NotOpen(port),
                    _ => ClosePortError::UPnPError,
                })?;
        };

        Ok(())
    }
}

#[derive(Error, Debug)]
pub enum GatewayError {
    #[error("Could not find local ip address")]
    LocalIpNotFound,
    #[error("Could not find upnp enabled gateway")]
    GatewayNotFound,
}

#[derive(Error, Debug)]
#[error("Could not operate upnp gateway to open port")]
pub struct OpenPortError;

#[derive(Error, Debug)]
pub enum ClosePortError {
    #[error("Could not close port {0}")]
    NotOpen(u16),
    #[error("Could not perform operation on gateway")]
    UPnPError,
}
