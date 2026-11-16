//! Trust is anchored in the TCP peer, never in an arbitrary forwarded chain.
use axum::http::HeaderMap;
use std::net::IpAddr;

pub fn resolve(peer: IpAddr, headers: &HeaderMap, trusted: &[IpAddr]) -> IpAddr {
    if !trusted.contains(&peer) {
        return peer;
    }
    let mut values = headers.get_all("x-real-ip").iter();
    let Some(value) = values.next() else {
        return peer;
    };
    if values.next().is_some() {
        return peer;
    }
    value
        .to_str()
        .ok()
        .and_then(|ip| ip.parse().ok())
        .unwrap_or(peer)
}
