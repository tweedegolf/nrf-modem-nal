use core::str::FromStr;

use crate::{at::AtSocket, error::Error, to_nb_result, Modem, SocketState};
use embedded_nal::nb;

impl Modem {
    pub fn lte_socket(&mut self) -> Result<LteSocket, Error> {
        Ok(LteSocket {
            inner: self.at_socket()?,
            state: SocketState::Closed,
        })
    }

    pub fn lte_connect(&mut self, socket: &mut LteSocket) -> nb::Result<(), Error> {
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

        socket.state = SocketState::Connected;

        self.at_connect(&mut socket.inner)?;

        Ok(())
    }

    pub fn lte_is_connected(&mut self, socket: &mut LteSocket) -> bool {
        socket.state.is_connected()
    }

    pub fn lte_read_clock(&mut self, socket: &mut LteSocket) -> Result<ClockTime, Error> {
        self.at_send(&mut socket.inner, "AT+CCLK?")?;

        let mut result = Err(Error::NoAtResponse);
        self.at_poll_response(&mut socket.inner, |s| {
            result = s.parse();
        })?;

        result
    }

    pub fn lte_close(&mut self, mut socket: LteSocket) -> Result<(), Error> {
        self.at_close(socket.inner)?;

        socket.state = SocketState::Closed;

        let mut new_state = self.state.clone();
        new_state.active_lte_sockets -= 1;
        self.change_state(new_state)?;

        Ok(())
    }
}

pub struct LteSocket {
    inner: AtSocket,
    state: SocketState,
}

pub struct ClockTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub sec: u8,
}

impl FromStr for ClockTime {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 29 {
            return Err(Error::UnexpectedAtResponse);
        }

        // Typical response: "+CCLK: \"18/12/06,22:10:00+08\""
        let time_str = &s[8..25];

        let year: i32 = time_str[0..2]
            .parse()
            .map_err(|_| Error::UnexpectedAtResponse)?;
        let month: u32 = time_str[3..5]
            .parse()
            .map_err(|_| Error::UnexpectedAtResponse)?;
        let day: u32 = time_str[6..8]
            .parse()
            .map_err(|_| Error::UnexpectedAtResponse)?;

        let hour: u32 = time_str[9..11]
            .parse()
            .map_err(|_| Error::UnexpectedAtResponse)?;
        let minute: u32 = time_str[12..14]
            .parse()
            .map_err(|_| Error::UnexpectedAtResponse)?;
        let sec: u32 = time_str[15..17]
            .parse()
            .map_err(|_| Error::UnexpectedAtResponse)?;

        Ok(ClockTime {
            year: year as _,
            month: month as _,
            day: day as _,
            hour: hour as _,
            minute: minute as _,
            sec: sec as _,
        })
    }
}
