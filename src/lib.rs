#![no_std]

use core::fmt::Write;
use embedded_nal::nb::{self};
use error::Error;

pub mod error;

pub use nrfxlib::{application_irq_handler, ipc_irq_handler, trace_irq_handler};
pub use embedded_nal;

pub struct Modem {
    state: ModemState,
}

impl Modem {
    pub fn new() -> Result<Self, Error> {
        nrfxlib::init()?;
        nrfxlib::modem::off()?;
        nrfxlib::modem::set_system_mode(nrfxlib::modem::SystemMode::LteMAndGnss)?;

        Ok(Self {
            state: ModemState::default(),
        })
    }

    pub fn debug(&self) -> impl core::fmt::Debug {
        self.state.clone()
    }

    fn change_state(&mut self, new_state: ModemState) -> Result<(), Error> {
        // Check what the LTE state should be
        match (self.state.active_lte_sockets, new_state.active_lte_sockets) {
            // Staying turned off
            (0, 0) => {}
            // Turning on
            (0, _) => {
                // Activate LTE without changing GNSS
                nrfxlib::at::send_at_command("AT+CFUN=21", |_| {})?;
            }
            // Turning off
            (_, 0) => {
                // Deactivate LTE without changing GNSS
                nrfxlib::at::send_at_command("AT+CFUN=20", |_| {})?;
            }
            // Staying turned on
            (_, _) => {}
        }

        // Check what the GNSS state should be
        match (
            self.state.active_gnss_sockets,
            new_state.active_gnss_sockets,
        ) {
            // Staying turned off
            (0, 0) => {}
            // Turning on
            (0, _) => {
                // Activate GNSS without changing LTE
                nrfxlib::at::send_at_command("AT+CFUN=31", |_| {})?;
            }
            // Turning off
            (_, 0) => {
                // Deactivate GNSS without changing LTE
                nrfxlib::at::send_at_command("AT+CFUN=30", |_| {})?;
            }
            // Staying turned on
            (_, _) => {}
        }

        self.state = new_state;
        Ok(())
    }

    fn wait_for_lte(&mut self) -> nb::Result<(), Error> {
        let mut values = None;
        to_nb_result(nrfxlib::at::send_at_command("AT+CEREG?", |val| {
            values = Some(
                at_commands::parser::CommandParser::parse(val.as_bytes())
                    .expect_identifier(b"+CEREG:")
                    .expect_int_parameter()
                    .expect_int_parameter()
                    .finish(),
            );
        }))?;

        if let Some(values) = values {
            let (_, stat) = to_nb_result(values)?;
            if stat == 1 || stat == 5 {
                Ok(())
            } else {
                Err(nb::Error::WouldBlock)
            }
        } else {
            to_nb_result(Err(Error::NoAtResponse))
        }
    }
}

impl embedded_nal::TcpClientStack for Modem {
    type TcpSocket = TcpSocket;
    type Error = Error;

    fn socket(&mut self) -> Result<Self::TcpSocket, Self::Error> {
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
        if matches!(socket.state, SocketState::Connected) {
            return nb::Result::Err(nb::Error::Other(Error::SocketAlreadyOpen));
        }

        if matches!(socket.state, SocketState::Closed) {
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

        Ok(())
    }

    fn is_connected(&mut self, socket: &Self::TcpSocket) -> Result<bool, Self::Error> {
        Ok(matches!(socket.state, SocketState::Connected))
    }

    fn send(
        &mut self,
        socket: &mut Self::TcpSocket,
        buffer: &[u8],
    ) -> nb::Result<usize, Self::Error> {
        if !self.is_connected(socket)? {
            return nb::Result::Err(nb::Error::Other(Error::SocketClosed));
        }

        match socket.inner.send(buffer) {
            Ok(Some(amount)) => nb::Result::Ok(amount),
            Ok(None) => nb::Result::Err(nb::Error::WouldBlock),
            Err(e) => nb::Result::Err(nb::Error::Other(e.into())),
        }
    }

    fn receive(
        &mut self,
        socket: &mut Self::TcpSocket,
        buffer: &mut [u8],
    ) -> nb::Result<usize, Self::Error> {
        if !self.is_connected(socket)? {
            return nb::Result::Err(nb::Error::Other(Error::SocketClosed));
        }

        match socket.inner.recv(buffer) {
            Ok(Some(amount)) => nb::Result::Ok(amount),
            Ok(None) => nb::Result::Err(nb::Error::WouldBlock),
            Err(e) => nb::Result::Err(nb::Error::Other(e.into())),
        }
    }

    fn close(&mut self, mut socket: Self::TcpSocket) -> Result<(), Self::Error> {
        socket.state = SocketState::Closed;
        drop(socket);

        let mut new_state = self.state.clone();
        new_state.active_lte_sockets -= 1;
        self.change_state(new_state)?;

        Ok(())
    }
}

fn to_nb_result<T, E1, E2: From<E1>>(r: Result<T, E1>) -> nb::Result<T, E2> {
    r.map_err(|e| nb::Error::Other(e.into()))
}

#[derive(Debug, Clone, Default)]
struct ModemState {
    active_lte_sockets: u32,
    active_gnss_sockets: u32,
}

pub struct TcpSocket {
    inner: nrfxlib::tcp::TcpSocket,
    state: SocketState,
}

impl Drop for TcpSocket {
    #[track_caller]
    fn drop(&mut self) {
        if !matches!(self.state, SocketState::Closed) {
            panic!("Sockets must be closed")
        }
    }
}

pub struct UdpSocket {
    inner: nrfxlib::udp::UdpSocket,
    state: SocketState,
}

impl Drop for UdpSocket {
    #[track_caller]
    fn drop(&mut self) {
        if !matches!(self.state, SocketState::Closed) {
            panic!("Sockets must be closed")
        }
    }
}

enum SocketState {
    Closed,
    WaitingForLte,
    Connected,
}
