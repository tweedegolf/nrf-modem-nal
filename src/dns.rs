use crate::{to_nb_result, Modem};
use core::str::FromStr;
use embedded_nal::{Ipv4Addr, Ipv6Addr};

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

            if result == core::ptr::null_mut() {
                return to_nb_result(Err(crate::Error::AddressNotFound));
            }

            'outer: while result != core::ptr::null_mut() {
                let address = &*(*result).ai_addr;

                if address.sa_family == nrfxlib_sys::NRF_AF_INET as i32 {
                    let address_data: &[u8] = address.sa_data.as_slice(address.sa_len as usize);
                    // So the address data can be much longer than the 4 bytes we need for the IP address
                    // And the address may not even start at the first byte!
                    // From what I've seen it can also start at the 4th byte.
                    // So we're going to chunk right through it and return the first global IP address we can find
                    for chunk_data in address_data.chunks_exact(4) {
                        found_ip = Some(embedded_nal::IpAddr::V4(Ipv4Addr::new(
                            chunk_data[0],
                            chunk_data[1],
                            chunk_data[2],
                            chunk_data[3],
                        )));
                        if found_ip.unwrap().is_global()
                            && address.sa_family == target_family as i32
                        {
                            break 'outer;
                        }
                    }
                } else if address.sa_family == nrfxlib_sys::NRF_AF_INET6 as i32 {
                    let address_data = address.sa_data.as_slice(address.sa_len as usize);
                    for chunk_data in address_data.chunks_exact(16) {
                        found_ip = Some(embedded_nal::IpAddr::V6(Ipv6Addr::new(
                            u16::from_be_bytes(chunk_data[0..2].try_into().unwrap()),
                            u16::from_be_bytes(chunk_data[2..4].try_into().unwrap()),
                            u16::from_be_bytes(chunk_data[4..6].try_into().unwrap()),
                            u16::from_be_bytes(chunk_data[6..8].try_into().unwrap()),
                            u16::from_be_bytes(chunk_data[8..10].try_into().unwrap()),
                            u16::from_be_bytes(chunk_data[10..12].try_into().unwrap()),
                            u16::from_be_bytes(chunk_data[12..14].try_into().unwrap()),
                            u16::from_be_bytes(chunk_data[14..16].try_into().unwrap()),
                        )));
                        if found_ip.unwrap().is_global()
                            && address.sa_family == target_family as i32
                        {
                            break 'outer;
                        }
                    }
                }

                result = (*result).ai_next;
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
