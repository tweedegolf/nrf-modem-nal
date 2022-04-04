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
