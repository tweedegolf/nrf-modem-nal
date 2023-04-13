#![doc = include_str!("../README.md")]
#![no_std]

use embedded_nal::nb;
use error::Error;

pub mod at;
pub mod dns;
pub mod error;
pub mod gnss;
pub mod helpers;
pub mod log;
pub mod lte;
pub mod tcp;
pub mod udp;

pub use embedded_nal;
pub use nrfxlib::{application_irq_handler, ipc_irq_handler, trace_irq_handler};

pub type GpsPowerCallback = fn(bool, &mut Modem) -> Result<(), Error>;

pub struct Modem {
    state: ModemState,
    gps_power_callback: GpsPowerCallback,
}

impl Modem {
    pub fn new(
        gps_power_callback: Option<GpsPowerCallback>,
        mode: SystemMode,
    ) -> Result<Self, Error> {
        nrfxlib::init()?;
        nrfxlib::modem::off()?;

        // nrfxlib::modem::set_system_mode(nrfxlib::modem::SystemMode::LteMAndGnss)?;

        let mut modem = Self {
            state: ModemState::default(),
            gps_power_callback: gps_power_callback.unwrap_or(|_, _| Ok(())),
        };

        modem.set_system_mode(mode)?;

        Ok(modem)
    }

    pub fn debug(&self) -> impl core::fmt::Debug {
        self.state.clone()
    }

    pub fn set_system_mode(&mut self, mode: SystemMode) -> Result<(), Error> {
        if !mode.is_valid_config() {
            return Err(Error::InvalidConfiguration);
        }

        let mut at = self.at_socket()?;

        let execute_result = (|| {
            self.at_connect(&mut at)?;
            let mut buffer = [0; 32];
            let data = mode.create_at_command(&mut buffer)?;
            self.at_send_raw(&mut at, data)?;
            Ok(())
        })();

        self.at_close(at)?;

        match execute_result {
            Err(Error::NrfModem(nrfxlib::Error::AtError(nrfxlib::AtError::CmeError(518)))) => {
                Err(Error::NotAllowedInActiveState)
            }
            Err(Error::NrfModem(nrfxlib::Error::AtError(nrfxlib::AtError::CmeError(522)))) => {
                Err(Error::InvalidBandConfiguration)
            }
            result => result,
        }
    }

    fn change_state(&mut self, new_state: ModemState) -> Result<(), Error> {
        log::debug!("New state: {:?}", new_state);

        // Check what the LTE state should be
        match (self.state.active_lte_sockets, new_state.active_lte_sockets) {
            // Staying turned off
            (0, 0) => {}
            // Turning on
            (0, _) => {
                log::debug!("Turning on modem lte");
                // Set Ultra low power mode.
                nrfxlib::at::send_at_command("AT%XDATAPRFL=0", |_| {})?;
                // Set UICC low power mode
                nrfxlib::at::send_at_command("AT+CEPPI=1", |_| {})?;
                // Set Power Saving Mode (PSM)
                nrfxlib::at::send_at_command("AT+CPSMS=1", |_| {})?;
                // Activate LTE without changing GNSS, this also activates UICC
                nrfxlib::at::send_at_command("AT+CFUN=21", |_| {})?;
            }
            // Turning off
            (_, 0) => {
                log::debug!("Turning off modem lte");
                // Deactivate LTE without changing GNSS
                nrfxlib::at::send_at_command("AT+CFUN=20", |_| {})?;
                // Deactivate UICC (Universal Integrated Circuit Card)
                nrfxlib::at::send_at_command("AT+CFUN=40", |_| {})?;
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
                log::debug!("Turning on modem gnss");
                (self.gps_power_callback)(true, self)?;
                // Activate GNSS without changing LTE
                nrfxlib::at::send_at_command("AT+CFUN=31", |_| {})?;
            }
            // Turning off
            (_, 0) => {
                log::debug!("Turning off modem gnss");
                // Deactivate GNSS without changing LTE
                (self.gps_power_callback)(false, self)?;
                nrfxlib::at::send_at_command("AT+CFUN=30", |_| {})?;
            }
            // Staying turned on
            (_, _) => {}
        }

        self.state = new_state;
        Ok(())
    }

    fn wait_for_lte(&mut self) -> nb::Result<(), Error> {
        log::trace!("Waiting for LTE");

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
            log::trace!("LTE status: {stat}");
            match stat {
                1 | 5 => Ok(()),
                0 | 2 | 4 => Err(nb::Error::WouldBlock),
                3 => to_nb_result(Err(Error::LteRegistrationDenied)),
                90 => to_nb_result(Err(Error::SimFailure)),
                _ => to_nb_result(Err(Error::UnexpectedAtResponse)),
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

#[derive(Debug, Clone, Copy)]
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

/// Identifies which radios in the nRF9160 should be active
///
/// Based on: <https://infocenter.nordicsemi.com/index.jsp?topic=%2Fref_at_commands%2FREF%2Fat_commands%2Fmob_termination_ctrl_status%2Fcfun.html>
#[derive(Debug, Copy, Clone)]
pub struct SystemMode {
    /// Enables the modem to connect to the LTE network
    pub lte_support: bool,
    /// Enables the modem to connect to the NBiot network
    pub nbiot_support: bool,
    /// Enables the modem to receive gnss signals
    pub gnss_support: bool,
    /// Sets up the preference the modem will have for connecting to the mobile network
    pub preference: ConnectionPreference,
}

/// The preference the modem will have for connecting to the mobile network
#[derive(Debug, Copy, Clone)]
pub enum ConnectionPreference {
    /// No preference. Initial system selection is based on history data and Universal Subscriber Identity Module (USIM)
    None = 0,
    /// LTE-M preferred
    Lte = 1,
    /// NB-IoT preferred
    Nbiot = 2,
    /// Network selection priorities override system priority, but if the same network or equal priority networks are found, LTE-M is preferred
    NetworkPreferenceWithLteFallback = 3,
    /// Network selection priorities override system priority, but if the same network or equal priority networks are found, NB-IoT is preferred
    NetworkPreferenceWithNbiotFallback = 4,
}

impl SystemMode {
    fn is_valid_config(&self) -> bool {
        match self.preference {
            ConnectionPreference::None => true,
            ConnectionPreference::Lte => self.lte_support,
            ConnectionPreference::Nbiot => self.nbiot_support,
            ConnectionPreference::NetworkPreferenceWithLteFallback => {
                self.lte_support && self.nbiot_support
            }
            ConnectionPreference::NetworkPreferenceWithNbiotFallback => {
                self.lte_support && self.nbiot_support
            }
        }
    }

    fn create_at_command<'a>(&self, buffer: &'a mut [u8]) -> Result<&'a [u8], Error> {
        at_commands::builder::CommandBuilder::create_set(buffer, true)
            .named("%XSYSTEMMODE")
            .with_int_parameter(self.lte_support as u8)
            .with_int_parameter(self.nbiot_support as u8)
            .with_int_parameter(self.gnss_support as u8)
            .with_int_parameter(self.preference as u8)
            .finish()
            .map_err(|e| Error::BufferTooSmall(Some(e)))
    }
}
