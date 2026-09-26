use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use subtle::ConstantTimeEq;
use super::{DEFAULT_DISCOVERY_PORT, DISCOVERY_MULTICAST_ADDR, DISCOVERY_BEACON_MAGIC};

const QUERY_MAGIC: [u8; 4] = *b"YSQ2";

pub(super) fn query(socket: &UdpSocket, id: &[u8; 32], local_ips: &[IpAddr]) {
    let mut packet = [0; 36];
    packet[..4].copy_from_slice(&QUERY_MAGIC);
    packet[4..].copy_from_slice(id);
    let _ = socket.send_to(&packet, (Ipv4Addr::BROADCAST, DEFAULT_DISCOVERY_PORT));
    let _ = socket.send_to(&packet, (DISCOVERY_MULTICAST_ADDR, DEFAULT_DISCOVERY_PORT));
    for ip in local_ips {
        if let IpAddr::V4(ip) = ip {
            let mut octets = ip.octets();
            octets[3] = 255;
            let _ = socket.send_to(&packet, (Ipv4Addr::from(octets), DEFAULT_DISCOVERY_PORT));
        }
    }
}

pub(super) fn responder() -> Option<UdpSocket> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, DEFAULT_DISCOVERY_PORT)).ok()?;
    socket.set_nonblocking(true).ok()?;
    let _ = socket.join_multicast_v4(&DISCOVERY_MULTICAST_ADDR.parse().ok()?, &Ipv4Addr::UNSPECIFIED);
    Some(socket)
}

pub(super) fn respond(socket: &UdpSocket, id: &[u8; 32], port: u16) {
    let mut packet = [0; 64];
    // Bound work per accept iteration; a UDP flood must not starve cancellation.
    for _ in 0..16 {
        let Ok((len, peer)) = socket.recv_from(&mut packet) else { break };
        if len != 36 || packet[..4] != QUERY_MAGIC || !bool::from(packet[4..36].ct_eq(id)) { continue; }
        let mut reply = [0; 38];
        reply[..4].copy_from_slice(&DISCOVERY_BEACON_MAGIC);
        reply[4..36].copy_from_slice(id);
        reply[36..].copy_from_slice(&port.to_be_bytes());
        let _ = socket.send_to(&reply, peer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discovery_replies_to_ephemeral_client_and_ignores_wrong_vault() {
        let host = UdpSocket::bind("127.0.0.1:0").unwrap();
        host.set_nonblocking(true).unwrap();
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.set_read_timeout(Some(std::time::Duration::from_millis(100))).unwrap();
        let mut query = [0; 36];
        query[..4].copy_from_slice(&QUERY_MAGIC);
        query[4..].copy_from_slice(&[8; 32]);
        client.send_to(&query, host.local_addr().unwrap()).unwrap();
        respond(&host, &[7; 32], 5678);
        assert!(client.recv(&mut [0; 64]).is_err());
        query[4..].copy_from_slice(&[7; 32]);
        client.send_to(&query, host.local_addr().unwrap()).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        respond(&host, &[7; 32], 5678);
        let mut response = [0; 38];
        assert_eq!(client.recv(&mut response).unwrap(), 38);
        assert_eq!(&response[..4], &DISCOVERY_BEACON_MAGIC);
        assert_eq!(u16::from_be_bytes([response[36], response[37]]), 5678);
    }
}
