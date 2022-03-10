#![no_std]

use embedded_nal::nb;
use error::Error;

pub mod at;
pub mod error;
pub mod gnss;
pub mod lte;
pub mod tcp;
pub mod udp;

pub use embedded_nal;
pub use nrfxlib::{application_irq_handler, ipc_irq_handler, trace_irq_handler};

pub struct Modem {
    state: ModemState,
    gps_power_callback: fn(bool, &mut Self),
}

impl Modem {
    pub fn new(gps_power_callback: Option<fn(bool, &mut Self)>) -> Result<Self, Error> {
        nrfxlib::init()?;
        nrfxlib::modem::off()?;
        nrfxlib::modem::set_system_mode(nrfxlib::modem::SystemMode::LteMAndGnss)?;

        Ok(Self {
            state: ModemState::default(),
            gps_power_callback: gps_power_callback.unwrap_or(|_, _| {}),
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
                (self.gps_power_callback)(true, self);
                nrfxlib::at::send_at_command("AT+CFUN=31", |_| {})?;
            }
            // Turning off
            (_, 0) => {
                // Deactivate GNSS without changing LTE
                (self.gps_power_callback)(false, self);
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

fn to_nb_result<T, E1, E2: From<E1>>(r: Result<T, E1>) -> nb::Result<T, E2> {
    r.map_err(|e| nb::Error::Other(e.into()))
}

#[derive(Debug, Clone, Default)]
struct ModemState {
    active_lte_sockets: u32,
    active_gnss_sockets: u32,
}

enum SocketState {
    Closed,
    WaitingForLte,
    Connected,
}

impl SocketState {
    /// Returns `true` if the socket state is [`Connected`].
    ///
    /// [`Connected`]: SocketState::Connected
    fn is_connected(&self) -> bool {
        matches!(self, Self::Connected)
    }

    /// Returns `true` if the socket state is [`Closed`].
    ///
    /// [`Closed`]: SocketState::Closed
    fn is_closed(&self) -> bool {
        matches!(self, Self::Closed)
    }
}
