use core::str::FromStr;

use embedded_nal::{Ipv4Addr, Ipv6Addr};
use crate::{to_nb_result, Modem};

impl embedded_nal::Dns for Modem {
    type Error = crate::Error;

    fn get_host_by_name(
        &mut self,
        hostname: &str,
        addr_type: embedded_nal::AddrType,
    ) -> embedded_nal::nb::Result<embedded_nal::IpAddr, Self::Error> {
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
                ai_socktype: nrfxlib_sys::NRF_SOCK_DGRAM as _,

                ai_flags: 0,
                ai_protocol: 0,
                ai_addrlen: 0,
                ai_addr: core::ptr::null_mut(),
                ai_canonname: core::ptr::null_mut(),
                ai_next: core::ptr::null_mut(),
            };

            let mut result: *mut nrfxlib_sys::nrf_addrinfo = core::ptr::null_mut();

            // A hostname should at most be 256 chars, but we have a null char as well, so we add one
            let mut hostname = heapless::String::<257>::from_str(hostname).map_err(|_| Self::Error::HostnameTooLong)?;
            hostname.push('\0').map_err(|_| Self::Error::HostnameTooLong)?;

            let err = nrfxlib_sys::nrf_getaddrinfo(
                hostname.as_ptr(),
                core::ptr::null(),
                &hints as *const _,
                &mut result as *mut *mut _,
            );

            if err != 0 {
                return to_nb_result(Err(crate::Error::NrfSys(nrfxlib::get_last_error())));
            }

            if result == core::ptr::null_mut() {
                return to_nb_result(Err(crate::Error::AddressNotFound));
            }

            while result != core::ptr::null_mut() {
                let address = &*(*result).ai_addr;

                if address.sa_family == nrfxlib_sys::NRF_AF_INET as i32 {
                    let address_data = address.sa_data.as_slice(address.sa_len as usize);
                    found_ip = Some(embedded_nal::IpAddr::V4(Ipv4Addr::new(
                        address_data[0],
                        address_data[1],
                        address_data[2],
                        address_data[3],
                    )));
                    if address.sa_family == target_family as i32 {
                        break;
                    }
                } else if address.sa_family == nrfxlib_sys::NRF_AF_INET6 as i32 {
                    let address_data = address.sa_data.as_slice(address.sa_len as usize);
                    found_ip = Some(embedded_nal::IpAddr::V6(Ipv6Addr::new(
                        u16::from_be_bytes(address_data[0..2].try_into().unwrap()),
                        u16::from_be_bytes(address_data[2..4].try_into().unwrap()),
                        u16::from_be_bytes(address_data[4..6].try_into().unwrap()),
                        u16::from_be_bytes(address_data[6..8].try_into().unwrap()),
                        u16::from_be_bytes(address_data[8..10].try_into().unwrap()),
                        u16::from_be_bytes(address_data[10..12].try_into().unwrap()),
                        u16::from_be_bytes(address_data[12..14].try_into().unwrap()),
                        u16::from_be_bytes(address_data[14..16].try_into().unwrap()),
                    )));
                    if address.sa_family == target_family as i32 {
                        break;
                    }
                } else {
                    result = (*result).ai_next;
                }
            }

            nrfxlib_sys::nrf_freeaddrinfo(result);

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
