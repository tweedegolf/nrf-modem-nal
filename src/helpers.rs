use crate::{
    at::AtSocket,
    gnss::{GnssOptions, GnssSocket},
    lte::LteSocket,
    Modem,
};
use embedded_nal::{nb, SocketAddr, TcpClientStack, UdpClientStack};
use nrfxlib::Pollable;

/// Perform a non-blocking write on the socket.
pub fn send(socket: &impl Pollable, buf: &[u8]) -> Result<Option<usize>, nrfxlib::Error> {
    let length = buf.len();
    let ptr = buf.as_ptr();
    let result = unsafe {
        nrfxlib_sys::nrf_send(
            socket.get_fd(),
            ptr as *const _,
            length as u32,
            nrfxlib_sys::NRF_MSG_DONTWAIT as i32,
        )
    };
    if result == -1 && nrfxlib::get_last_error() == nrfxlib_sys::NRF_EAGAIN as i32 {
        // This is EAGAIN
        Ok(None)
    } else if result < 0 {
        Err(nrfxlib::Error::Nordic(
            "send",
            result as i32,
            nrfxlib::get_last_error(),
        ))
    } else {
        Ok(Some(result as usize))
    }
}

/// Creates a new socket, lets it connect, hands it over to the given function, closes the socket and then returns the function result.
/// This makes sure that closing the socket is not forgotten.
pub fn deferred_tcp_socket<NET, F, R, E>(
    net: &mut NET,
    remote: SocketAddr,
    function: F,
) -> Result<R, E>
where
    NET: TcpClientStack,
    F: FnOnce(&mut NET, &mut NET::TcpSocket) -> Result<R, E>,
    E: From<NET::Error>,
{
    let mut socket = net.socket()?;

    let result = nb::block!(net.connect(&mut socket, remote))
        .map_err(|e| e.into())
        .and_then(|_| function(net, &mut socket));

    net.close(socket)?;

    result
}

/// Creates a new socket, lets it connect, hands it over to the given function, closes the socket and then returns the function result.
/// This makes sure that closing the socket is not forgotten.
pub fn deferred_udp_socket<NET, F, R, E>(
    net: &mut NET,
    remote: SocketAddr,
    function: F,
) -> Result<R, E>
where
    NET: UdpClientStack,
    F: FnOnce(&mut NET, &mut NET::UdpSocket) -> Result<R, E>,
    E: From<NET::Error>,
{
    let mut socket = net.socket()?;

    let result = net
        .connect(&mut socket, remote)
        .map_err(|e| e.into())
        .and_then(|_| function(net, &mut socket));

    net.close(socket)?;

    result
}

/// Creates a new socket, lets it connect, hands it over to the given function, closes the socket and then returns the function result.
/// This makes sure that closing the socket is not forgotten.
pub fn deferred_at_socket<F, R, E>(net: &mut Modem, function: F) -> Result<R, E>
where
    F: FnOnce(&mut Modem, &mut AtSocket) -> Result<R, E>,
    E: From<crate::error::Error>,
{
    let mut socket = net.at_socket()?;

    let result = net
        .at_connect(&mut socket)
        .map_err(|e| e.into())
        .and_then(|_| function(net, &mut socket));

    net.at_close(socket)?;

    result
}

/// Creates a new socket, lets it connect, hands it over to the given function, closes the socket and then returns the function result.
/// This makes sure that closing the socket is not forgotten.
pub fn deferred_lte_socket<F, R, E>(net: &mut Modem, function: F) -> Result<R, E>
where
    F: FnOnce(&mut Modem, &mut LteSocket) -> Result<R, E>,
    E: From<crate::error::Error>,
{
    let mut socket = net.lte_socket()?;

    let result = nb::block!(net.lte_connect(&mut socket))
        .map_err(|e| e.into())
        .and_then(|_| function(net, &mut socket));

    net.lte_close(socket)?;

    result
}

/// Creates a new socket, lets it connect, hands it over to the given function, closes the socket and then returns the function result.
/// This makes sure that closing the socket is not forgotten.
pub fn deferred_gnss_socket<F, R, E>(
    net: &mut Modem,
    options: GnssOptions,
    function: F,
) -> Result<R, E>
where
    F: FnOnce(&mut Modem, &mut GnssSocket) -> Result<R, E>,
    E: From<crate::error::Error>,
{
    let mut socket = net.gnss_socket()?;

    let result = net
        .gnss_connect(&mut socket, options)
        .map_err(|e| e.into())
        .and_then(|_| function(net, &mut socket));

    net.gnss_close(socket)?;

    result
}
