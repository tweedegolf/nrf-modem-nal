use crate::{error::Error, log, Modem, SocketState};
use core::fmt::Write;
use embedded_nal::{
    nb::{self},
    SocketAddr,
};

impl embedded_nal::UdpClientStack for Modem {
    type UdpSocket = UdpSocket;
    type Error = Error;

    fn socket(&mut self) -> Result<Self::UdpSocket, Self::Error> {
        log::debug!("Creating UDP socket");

        Ok(UdpSocket {
            inner: nrfxlib::udp::UdpSocket::new()?,
            state: SocketState::Closed,
            remote_address: None,
        })
    }

    fn connect(
        &mut self,
        socket: &mut Self::UdpSocket,
        remote: embedded_nal::SocketAddr,
    ) -> Result<(), Self::Error> {
        log::trace!("Connecting UDP socket to {}", remote);

        if socket.state.is_connected() {
            return Err(Error::SocketAlreadyOpen);
        }

        if socket.state.is_closed() {
            let mut new_state = self.state.clone();
            new_state.active_lte_sockets += 1;
            self.change_state(new_state)?;
            socket.state = SocketState::WaitingForLte;
        }

        nb::block!(self.wait_for_lte())?;

        let mut ip_string = heapless::String::<64>::new();
        write!(ip_string, "{}", remote.ip())?;

        socket.inner.connect(&ip_string, remote.port())?;
        socket.state = SocketState::Connected;
        socket.remote_address = Some(remote);

        log::debug!("Connected UDP socket");

        Ok(())
    }

    fn send(&mut self, socket: &mut Self::UdpSocket, buffer: &[u8]) -> nb::Result<(), Self::Error> {
        log::trace!("Sending to UDP socket");

        if !socket.state.is_connected() {
            return nb::Result::Err(nb::Error::Other(Error::SocketClosed));
        }

        match crate::helpers::send(&socket.inner, buffer) {
            Ok(Some(_)) => {
                log::debug!("Sent {} bytes from UDP socket", buffer.len());
                nb::Result::Ok(())
            },
            Ok(None) => nb::Result::Err(nb::Error::WouldBlock),
            Err(e) => nb::Result::Err(nb::Error::Other(e.into())),
        }
    }

    fn receive(
        &mut self,
        socket: &mut Self::UdpSocket,
        buffer: &mut [u8],
    ) -> nb::Result<(usize, embedded_nal::SocketAddr), Self::Error> {
        log::trace!("Receiving from UDP socket");

        if !socket.state.is_connected() {
            return nb::Result::Err(nb::Error::Other(Error::SocketClosed));
        }

        match socket.inner.recv(buffer) {
            Ok(Some(amount)) => {
                log::debug!("Received {amount} bytes from UDP socket");
                nb::Result::Ok((amount, socket.remote_address.unwrap()))
            },
            Ok(None) => nb::Result::Err(nb::Error::WouldBlock),
            Err(e) => nb::Result::Err(nb::Error::Other(e.into())),
        }
    }

    fn close(&mut self, mut socket: Self::UdpSocket) -> Result<(), Self::Error> {
        log::debug!("Closing UDP socket");

        let socket_state = socket.state;

        socket.state = SocketState::Closed;
        drop(socket);

        if !socket_state.is_closed() {
            let mut new_state = self.state.clone();
            new_state.active_lte_sockets -= 1;
            self.change_state(new_state)?;
        }

        Ok(())
    }
}

pub struct UdpSocket {
    inner: nrfxlib::udp::UdpSocket,
    state: SocketState,
    remote_address: Option<SocketAddr>,
}

impl Drop for UdpSocket {
    #[track_caller]
    fn drop(&mut self) {
        if !self.state.is_closed() {
            panic!("Sockets must be closed")
        }
    }
}
