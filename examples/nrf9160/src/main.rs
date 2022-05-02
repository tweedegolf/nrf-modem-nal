#![no_main]
#![no_std]

// links in a minimal version of libc
extern crate tinyrlibc;

use hal::{
    pac::{self, interrupt},
    prelude::*,
};
use nrf9160_hal as hal;
use nrf_modem_nal::{
    embedded_nal::{nb, Dns, SocketAddr, TcpClientStack},
    gnss::{GnssData, GnssOptions},
    ConnectionPreference, Modem, SystemMode,
};
use rtt_target::{rprintln, rtt_init_print};

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();

    let mut cp = cortex_m::Peripherals::take().unwrap();
    let dp = nrf9160_hal::pac::Peripherals::take().unwrap();

    let pins0 = hal::gpio::p0::Parts::new(dp.P0_NS);
    // Turn the LED on
    let mut led_blue = pins0
        .p0_03
        .into_push_pull_output(hal::gpio::Level::Low)
        .degrade();

    // Enable the modem interrupts
    unsafe {
        pac::NVIC::unmask(pac::Interrupt::EGU1);
        pac::NVIC::unmask(pac::Interrupt::EGU2);
        pac::NVIC::unmask(pac::Interrupt::IPC);
        cp.NVIC.set_priority(pac::Interrupt::EGU1, 4 << 5);
        cp.NVIC.set_priority(pac::Interrupt::EGU2, 4 << 5);
        cp.NVIC.set_priority(pac::Interrupt::IPC, 0 << 5);
    }

    rprintln!("Initializing modem");
    let mut modem = nrf_modem_nal::Modem::new(
        None,
        SystemMode {
            gnss_support: false,
            lte_support: false,
            nbiot_support: true,
            preference: ConnectionPreference::Nbiot,
        },
    )
    .unwrap();

    let mut at = modem.at_socket().unwrap();
    modem.at_connect(&mut at).unwrap();
    modem.at_send(&mut at, "AT+CNEC=24").unwrap();
    let mut buffer = [0; 1024];
    nb::block!(modem.at_receive(&mut at, &mut buffer)).unwrap();

    let mut lte = modem.lte_socket().unwrap();
    rprintln!("Connecting to lte");
    loop {
        match modem.at_receive(&mut at, &mut buffer) {
            Ok(count) => {
                rprintln!("AT:  {}", core::str::from_utf8(&buffer[..count]).unwrap());
                panic!("Test");
            }
            _ => {}
        }

        match modem.lte_connect(&mut lte) {
            Ok(val) => break Ok(val),
            Err(nb::Error::WouldBlock) => continue,
            Err(e) => break Err(e),
        }
    }
    .unwrap();
    rprintln!("Done");

    do_dns(&mut modem);
    do_tcp(&mut modem);
    // do_gnss(&mut modem);

    led_blue.set_high().unwrap();

    loop {
        cortex_m::asm::bkpt();
    }
}

fn do_tcp(modem: &mut Modem) {
    rprintln!("Creating TCP socket: {:?}", modem.debug());
    let mut tcp_socket = modem.socket().unwrap();

    rprintln!("Connect TCP socket: {:?}", modem.debug());
    nb::block!(modem.connect(
        &mut tcp_socket,
        SocketAddr::V4("142.250.179.211:80".parse().unwrap())
    ))
    .unwrap(); // ip.jsontest.com

    rprintln!("Sending TCP socket: {:?}", modem.debug());
    nb::block!(modem.send(
        &mut tcp_socket,
        "GET / HTTP/1.0\nHost: ip.jsontest.com\r\n\r\n".as_bytes()
    ))
    .unwrap();

    rprintln!("Receiving TCP socket: {:?}", modem.debug());
    let mut buffer = [0; 1024];
    let received_length = nb::block!(modem.receive(&mut tcp_socket, &mut buffer)).unwrap();

    rprintln!(
        "Received: {}",
        core::str::from_utf8(&buffer[..received_length]).unwrap()
    );

    rprintln!("Closing TCP socket: {:?}", modem.debug());
    modem.close(tcp_socket).unwrap();

    rprintln!("End: {:?}", modem.debug());
}

fn do_gnss(modem: &mut Modem) {
    rprintln!("Creating Gnss socket: {:?}", modem.debug());
    let mut gnss_socket = modem.gnss_socket().unwrap();

    rprintln!("Connect Gnss socket: {:?}", modem.debug());
    modem
        .gnss_connect(&mut gnss_socket, GnssOptions::default())
        .unwrap();

    for _ in 0..10 {
        rprintln!("Receiving Gnss socket: {:?}", modem.debug());
        let received = nb::block!(modem.gnss_receive(&mut gnss_socket)).unwrap();
        match received {
            GnssData::Nmea { buffer, length } => {
                let nmea_str = unsafe { core::str::from_utf8_unchecked(&buffer[0..length]) };
                rprintln!("Received nmea: {}", nmea_str);
            }
            GnssData::Position(p) => rprintln!("Received position: lat: {}°, long: {}°, alt: {}m, acc: {}m, speed: {}m/s, heading: {}°", p.latitude, p.longitude, p.altitude, p.accuracy, p.speed, p.heading),
            GnssData::Agps(_) => rprintln!("Received Agps: "),
        }
    }

    rprintln!("Closing Gnss socket: {:?}", modem.debug());
    modem.gnss_close(gnss_socket).unwrap();

    rprintln!("End: {:?}", modem.debug());
}

fn do_dns(modem: &mut Modem) {
    rprintln!(
        "Dns for tweedegolf.nl: {:?}",
        nb::block!(
            modem.get_host_by_name("tweedegolf.nl", nrf_modem_nal::embedded_nal::AddrType::IPv4)
        )
    );
}

#[link_section = ".spm"]
#[used]
static SPM: [u8; 24052] = *include_bytes!("zephyr.bin");

/// Interrupt Handler for LTE related hardware. Defer straight to the library.
#[interrupt]
fn EGU1() {
    nrf_modem_nal::application_irq_handler();
    cortex_m::asm::sev();
}

/// Interrupt Handler for LTE related hardware. Defer straight to the library.
#[interrupt]
fn EGU2() {
    nrf_modem_nal::trace_irq_handler();
    cortex_m::asm::sev();
}

/// Interrupt Handler for LTE related hardware. Defer straight to the library.
#[interrupt]
fn IPC() {
    nrf_modem_nal::ipc_irq_handler();
    cortex_m::asm::sev();
}

#[cortex_m_rt::exception]
unsafe fn HardFault(frame: &cortex_m_rt::ExceptionFrame) -> ! {
    panic!("{:?}", frame);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    rprintln!("{}", info);
    loop {
        cortex_m::asm::udf();
    }
}
