use crate::{error::Error, Modem, SocketState};
use embedded_nal::nb;

impl Modem {
    pub fn at_socket(&mut self) -> Result<AtSocket, Error> {
        Ok(AtSocket {
            inner: nrfxlib::at::AtSocket::new()?,
            state: SocketState::Closed,
        })
    }

    pub fn at_connect(&mut self, socket: &mut AtSocket) -> Result<(), Error> {
        if socket.state.is_connected() {
            return Err(Error::SocketAlreadyOpen);
        }

        socket.state = SocketState::Connected;

        Ok(())
    }

    pub fn at_send(&mut self, socket: &mut AtSocket, data: &str) -> Result<(), Error> {
        if !socket.state.is_connected() {
            return Err(Error::SocketClosed);
        }

        socket.inner.send_command(data)?;

        Ok(())
    }

    pub fn at_send_raw(&mut self, socket: &mut AtSocket, data: &[u8]) -> Result<(), Error> {
        if !socket.state.is_connected() {
            return Err(Error::SocketClosed);
        }

        socket.inner.write(data)?;

        Ok(())
    }

    pub fn at_poll_response<F>(
        &mut self,
        socket: &mut AtSocket,
        callback_function: F,
    ) -> Result<(), Error>
    where
        F: FnMut(&str),
    {
        socket.inner.poll_response(callback_function)?;
        Ok(())
    }

    pub fn at_receive(
        &mut self,
        socket: &mut AtSocket,
        buffer: &mut [u8],
    ) -> nb::Result<usize, Error> {
        if !socket.state.is_connected() {
            return nb::Result::Err(nb::Error::Other(Error::SocketClosed));
        }

        match socket.inner.recv(buffer) {
            Ok(Some(amount)) => nb::Result::Ok(amount),
            Ok(None) => nb::Result::Err(nb::Error::WouldBlock),
            Err(e) => nb::Result::Err(nb::Error::Other(e.into())),
        }
    }

    pub fn at_close(&mut self, mut socket: AtSocket) -> Result<(), Error> {
        socket.state = SocketState::Closed;
        drop(socket);

        let mut new_state = self.state.clone();
        new_state.active_lte_sockets -= 1;
        self.change_state(new_state)?;

        Ok(())
    }
}

pub struct AtSocket {
    inner: nrfxlib::at::AtSocket,
    state: SocketState,
}

impl Drop for AtSocket {
    #[track_caller]
    fn drop(&mut self) {
        if !self.state.is_closed() {
            panic!("Sockets must be closed")
        }
    }
}
