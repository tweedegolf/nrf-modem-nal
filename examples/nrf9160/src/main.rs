#![no_main]
#![no_std]

// links in a minimal version of libc
extern crate tinyrlibc;

use hal::pac::{self, interrupt};
use nrf9160_hal as hal;
use nrf_modem_nal::embedded_nal::{TcpClientStack, SocketAddr, nb};
use rtt_target::{rtt_init_print, rprintln};

#[cortex_m_rt::entry]
fn main() -> ! {
    rtt_init_print!();

    let mut cp = cortex_m::Peripherals::take().unwrap();
    let _dp = nrf9160_hal::pac::Peripherals::take().unwrap();

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
    let mut modem = nrf_modem_nal::Modem::new().unwrap();

    rprintln!("Creating TCP socket: {:?}", modem.debug());
    let mut tcp_socket = modem.socket().unwrap();

    rprintln!("Connect TCP socket: {:?}", modem.debug());
    nb::block!(modem.connect(&mut tcp_socket, SocketAddr::V4("52.216.99.90:80".parse().unwrap()))).unwrap();

    rprintln!("Sending TCP socket: {:?}", modem.debug());
    nb::block!(modem.send(&mut tcp_socket, "GET / HTTP/1.0\r\n\r\n".as_bytes())).unwrap();

    rprintln!("Receiving TCP socket: {:?}", modem.debug());
    let mut buffer = [0; 1024];
    let received_length = nb::block!(modem.receive(&mut tcp_socket, &mut buffer)).unwrap();

    rprintln!("Received: {}", core::str::from_utf8(&buffer[..received_length]).unwrap());

    rprintln!("Closing TCP socket: {:?}", modem.debug());
    modem.close(tcp_socket).unwrap();

    rprintln!("End: {:?}", modem.debug());

    loop {
        cortex_m::asm::bkpt();
    }
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
        cortex_m::asm::bkpt();
    }
}
