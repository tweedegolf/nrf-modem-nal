use crate::{error::Error, to_nb_result, Modem, SocketState};
use embedded_nal::nb;

pub use nrfxlib::gnss::{DeleteMask, GnssData, NmeaMask};

impl Modem {
    pub fn gnss_socket(&mut self) -> Result<GnssSocket, Error> {
        Ok(GnssSocket {
            inner: nrfxlib::gnss::GnssSocket::new()?,
            state: SocketState::Closed,
        })
    }

    pub fn gnss_connect(
        &mut self,
        socket: &mut GnssSocket,
        options: GnssOptions,
    ) -> Result<(), Error> {
        if socket.state.is_connected() {
            return Err(Error::SocketAlreadyOpen);
        }

        let mut new_state = self.state.clone();
        new_state.active_gnss_sockets += 1;
        self.change_state(new_state)?;

        socket.inner.set_fix_interval(options.fix_interval)?;
        socket.inner.set_fix_retry(options.fix_retry)?;
        socket.inner.set_nmea_mask(options.nmea_mask)?;

        socket.inner.start(options.delete_mask)?;
        socket.state = SocketState::Connected;

        Ok(())
    }

    pub fn gnss_receive(&mut self, socket: &mut GnssSocket) -> nb::Result<GnssData, Error> {
        if !socket.state.is_connected() {
            return nb::Result::Err(nb::Error::Other(Error::SocketClosed));
        }

        let fix = to_nb_result(socket.inner.get_fix())?;

        match fix {
            Some(fix) => Ok(fix),
            None => Err(nb::Error::WouldBlock),
        }
    }

    pub fn gnss_close(&mut self, mut socket: GnssSocket) -> Result<(), Error> {
        socket.state = SocketState::Closed;
        drop(socket);

        let mut new_state = self.state.clone();
        new_state.active_gnss_sockets -= 1;
        self.change_state(new_state)?;

        Ok(())
    }
}

pub struct GnssSocket {
    inner: nrfxlib::gnss::GnssSocket,
    state: SocketState,
}

impl Drop for GnssSocket {
    #[track_caller]
    fn drop(&mut self) {
        if !self.state.is_closed() {
            panic!("Sockets must be closed")
        }
    }
}

pub struct GnssOptions {
    pub delete_mask: DeleteMask,
    pub fix_interval: u16,
    pub fix_retry: u16,
    pub nmea_mask: NmeaMask,
}

impl Default for GnssOptions {
    fn default() -> Self {
        Self {
            delete_mask: Default::default(),
            fix_interval: 1,
            fix_retry: 60,
            nmea_mask: Default::default(),
        }
    }
}
