use crate::{log, to_nb_result, Modem};
use core::str::FromStr;
use embedded_nal::{Ipv4Addr, Ipv6Addr};

impl embedded_nal::Dns for Modem {
    type Error = crate::Error;

    fn get_host_by_name(
        &mut self,
        hostname: &str,
        addr_type: embedded_nal::AddrType,
    ) -> embedded_nal::nb::Result<embedded_nal::IpAddr, Self::Error> {
        log::info!("Resolving dns hostname for \"{}\"", hostname);

        if let Ok(ip) = hostname.parse() {
            return Ok(ip);
        }

        if !hostname.is_ascii() {
            return to_nb_result(Err(Self::Error::HostnameNotAscii));
        }

        let target_family = match addr_type {
            embedded_nal::AddrType::IPv4 => nrfxlib_sys::NRF_AF_INET,
            embedded_nal::AddrType::IPv6 => nrfxlib_sys::NRF_AF_INET6,
            embedded_nal::AddrType::Either => nrfxlib_sys::NRF_AF_INET,
        };

        let mut found_ip = None;

        unsafe {
            let hints = nrfxlib_sys::nrf_addrinfo {
                ai_family: target_family as _,
                ai_socktype: nrfxlib_sys::NRF_SOCK_STREAM as _,

                ai_flags: 0,
                ai_protocol: 0,
                ai_addrlen: 0,
                ai_addr: core::ptr::null_mut(),
                ai_canonname: core::ptr::null_mut(),
                ai_next: core::ptr::null_mut(),
            };

            let mut result: *mut nrfxlib_sys::nrf_addrinfo = core::ptr::null_mut();

            // A hostname should at most be 256 chars, but we have a null char as well, so we add one
            let mut hostname = heapless::String::<257>::from_str(hostname)
                .map_err(|_| Self::Error::HostnameTooLong)?;
            hostname
                .push('\0')
                .map_err(|_| Self::Error::HostnameTooLong)?;

            let err = nrfxlib_sys::nrf_getaddrinfo(
                hostname.as_ptr(),
                core::ptr::null(),
                &hints as *const _,
                &mut result as *mut *mut _,
            );

            if err != 0 {
                return to_nb_result(Err(crate::Error::NrfSys(nrfxlib::get_last_error())));
            }

            if result.is_null() {
                return to_nb_result(Err(crate::Error::AddressNotFound));
            }

            let mut result_iter = result;

            while !result_iter.is_null() && found_ip.is_none() {
                let address = (*result_iter).ai_addr;

                if (*address).sa_family == nrfxlib_sys::NRF_AF_INET as i32 {
                    let dns_addr: &nrfxlib_sys::nrf_sockaddr_in =
                        &*(address as *const nrfxlib_sys::nrf_sockaddr_in);

                    found_ip = Some(embedded_nal::IpAddr::V4(Ipv4Addr::from(
                        dns_addr.sin_addr.s_addr.to_ne_bytes(),
                    )));
                } else if (*address).sa_family == nrfxlib_sys::NRF_AF_INET6 as i32 {
                    let dns_addr: &nrfxlib_sys::nrf_sockaddr_in6 =
                        &*(address as *const nrfxlib_sys::nrf_sockaddr_in6);

                    found_ip = Some(embedded_nal::IpAddr::V6(Ipv6Addr::from(
                        dns_addr.sin6_addr.s6_addr,
                    )));
                }

                result_iter = (*result_iter).ai_next;
            }

            nrfxlib_sys::nrf_freeaddrinfo(result);

            log::info!("{found_ip:?}");

            if let Some(found_ip) = found_ip {
                Ok(found_ip)
            } else {
                to_nb_result(Err(crate::Error::AddressNotFound))
            }
        }
    }

    fn get_host_by_address(
        &mut self,
        _addr: embedded_nal::IpAddr,
    ) -> embedded_nal::nb::Result<heapless::String<256>, Self::Error> {
        unimplemented!()
    }
}
