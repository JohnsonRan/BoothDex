//! The upstream client's DNS resolver. It keeps only publicly routable addresses, so a DNS answer for an upstream
//! host cannot aim the proxy at itself, its own network or a cloud metadata service.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

/// The system resolver (getaddrinfo, as reqwest's default), less every address `is_public` refuses. A name with no
/// public address fails to resolve, so the request fails before it connects.
pub(crate) struct PublicOnly;

impl Resolve for PublicOnly {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_owned();
        Box::pin(async move {
            let addrs: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0)).await?.filter(|a| is_public(a.ip())).collect();
            if addrs.is_empty() {
                return Err(format!("{host} resolves to no public address").into());
            }
            Ok(Box::new(addrs.into_iter()) as Addrs)
        })
    }
}

fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(a) => is_public_v4(a),
        IpAddr::V6(a) => a.to_ipv4_mapped().map_or_else(|| is_public_v6(a), is_public_v4),
    }
}

/// Every IPv4 address outside the special-purpose blocks of RFC 6890 and its updates.
fn is_public_v4(a: Ipv4Addr) -> bool {
    let [b0, b1, b2, _] = a.octets();
    !(b0 == 0 // "this network"
        || a.is_private()
        || a.is_loopback()
        || a.is_link_local() // the cloud metadata services among them
        || (b0 == 100 && b1 & 0xc0 == 64) // 100.64.0.0/10, carrier-grade NAT
        || (b0 == 192 && b1 == 0 && b2 == 0) // 192.0.0.0/24, IETF protocol assignments
        || a.is_documentation()
        || (b0 == 198 && b1 & 0xfe == 18) // 198.18.0.0/15, benchmarking
        || b0 >= 224) // multicast, reserved and broadcast
}

/// Global unicast (2000::/3) less its special-purpose blocks. NAT64 (64:ff9b::/96) falls outside 2000::/3 and is
/// refused, so an IPv6-only host behind DNS64 cannot reach the upstreams; the container networks have IPv4.
fn is_public_v6(a: Ipv6Addr) -> bool {
    let s = a.segments();
    s[0] & 0xe000 == 0x2000
        && !(s[0] == 0x2001 && s[1] < 0x200) // 2001::/23, IETF protocol assignments, Teredo among them
        && !(s[0] == 0x2001 && s[1] == 0xdb8) // documentation
        && s[0] != 0x2002 // 6to4, which carries any IPv4 address
        && !(s[0] == 0x3fff && s[1] < 0x1000) // 3fff::/20, documentation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_public_addresses_pass() {
        let public = [
            "1.1.1.1", "8.8.8.8", "104.16.0.1", "100.63.255.255", "100.128.0.1", "172.32.0.1", "192.0.1.1", "198.20.0.1", "223.255.255.255",
            "2606:4700::1", "2001:4860::8888", "2001:200::1", "::ffff:8.8.8.8",
        ];
        let refused = [
            "0.0.0.0", "0.1.2.3", "10.1.2.3", "127.0.0.1", "127.255.0.1", "172.16.0.1", "172.31.255.255", "192.168.1.1", "169.254.169.254",
            "100.64.0.1", "100.127.255.255", "192.0.0.1", "192.0.2.1", "198.51.100.1", "203.0.113.1", "198.18.0.1", "198.19.255.255",
            "224.0.0.1", "240.0.0.1", "255.255.255.255", "::", "::1", "::127.0.0.1", "::ffff:127.0.0.1", "::ffff:169.254.169.254",
            "::ffff:10.0.0.1", "64:ff9b::7f00:1", "fc00::1", "fd00:ec2::254", "fe80::1", "ff02::1", "2001::1", "2001:1ff::1",
            "2001:db8::1", "2002:7f00:1::1", "3fff::1",
        ];
        for ip in public {
            assert!(is_public(ip.parse().unwrap()), "{ip} should pass");
        }
        for ip in refused {
            assert!(!is_public(ip.parse().unwrap()), "{ip} should be refused");
        }
    }

    #[tokio::test]
    async fn a_name_with_only_loopback_addresses_does_not_resolve() {
        assert!(PublicOnly.resolve("localhost".parse().unwrap()).await.is_err());
    }
}
