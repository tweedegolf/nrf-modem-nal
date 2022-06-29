use crate::{error::Error, log, to_nb_result, Modem, SocketState};
use core::fmt::Write;
use embedded_nal::nb::{self};

impl embedded_nal::TcpClientStack for Modem {
    type TcpSocket = TcpSocket;
    type Error = Error;

    fn socket(&mut self) -> Result<Self::TcpSocket, Self::Error> {
        log::debug!("Creating TCP socket");

        Ok(TcpSocket {
            inner: nrfxlib::tcp::TcpSocket::new()?,
            state: SocketState::Closed,
        })
    }

    fn connect(
        &mut self,
        socket: &mut Self::TcpSocket,
        remote: embedded_nal::SocketAddr,
    ) -> nb::Result<(), Self::Error> {
        log::trace!("Connecting TCP socket to {}", remote);

        if socket.state.is_connected() {
            return nb::Result::Err(nb::Error::Other(Error::SocketAlreadyOpen));
        }

        if socket.state.is_closed() {
            let mut new_state = self.state.clone();
            new_state.active_lte_sockets += 1;
            to_nb_result(self.change_state(new_state))?;
            socket.state = SocketState::WaitingForLte;
        }

        self.wait_for_lte()?;

        let mut ip_string = heapless::String::<64>::new();
        to_nb_result(write!(ip_string, "{}", remote.ip()))?;

        to_nb_result(socket.inner.connect(&ip_string, remote.port()))?;
        socket.state = SocketState::Connected;

        log::debug!("Connected TCP socket");

        Ok(())
    }

    fn is_connected(&mut self, socket: &Self::TcpSocket) -> Result<bool, Self::Error> {
        Ok(socket.state.is_connected())
    }

    fn send(
        &mut self,
        socket: &mut Self::TcpSocket,
        buffer: &[u8],
    ) -> nb::Result<usize, Self::Error> {
        log::trace!("Sending to TCP socket");

        if !socket.state.is_connected() {
            return nb::Result::Err(nb::Error::Other(Error::SocketClosed));
        }

        match crate::helpers::send(&socket.inner, buffer) {
            Ok(Some(amount)) => {
                log::debug!("Sent {amount} bytes to TCP socket");
                nb::Result::Ok(amount)
            },
            Ok(None) => nb::Result::Err(nb::Error::WouldBlock),
            Err(e) => nb::Result::Err(nb::Error::Other(e.into())),
        }
    }

    fn receive(
        &mut self,
        socket: &mut Self::TcpSocket,
        buffer: &mut [u8],
    ) -> nb::Result<usize, Self::Error> {
        log::trace!("Receiving from TCP socket");

        if !socket.state.is_connected() {
            return nb::Result::Err(nb::Error::Other(Error::SocketClosed));
        }

        match socket.inner.recv(buffer) {
            Ok(Some(amount)) => {
                log::debug!("Received {amount} bytes from TCP socket");
                nb::Result::Ok(amount)
            },
            Ok(None) => nb::Result::Err(nb::Error::WouldBlock),
            Err(e) => nb::Result::Err(nb::Error::Other(e.into())),
        }
    }

    fn close(&mut self, mut socket: Self::TcpSocket) -> Result<(), Self::Error> {
        log::debug!("Closing TCP socket");

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

pub struct TcpSocket {
    inner: nrfxlib::tcp::TcpSocket,
    state: SocketState,
}

impl Drop for TcpSocket {
    #[track_caller]
    fn drop(&mut self) {
        if !self.state.is_closed() {
            panic!("Sockets must be closed")
        }
    }
}
