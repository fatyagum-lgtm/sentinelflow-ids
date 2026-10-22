use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Outbound,
    Inbound,
    Internal,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub id: String,
    pub timestamp_ms: u64,
    pub source: IpAddr,
    pub destination: IpAddr,
    pub source_port: u16,
    pub destination_port: u16,
    pub protocol: Protocol,
    pub packets: u64,
    pub bytes: u64,
    pub direction: Direction,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub tcp_flags: Option<u8>,
}
impl Event {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        {
            return Err("id must be 1–64 ASCII letters, digits, _ or -");
        }
        if !(946684800000..4102444800000).contains(&self.timestamp_ms) {
            return Err("timestamp_ms must be Unix UTC milliseconds in 2000–2099");
        }
        if self.packets == 0 || self.packets > 10_000_000 || self.bytes > 1_000_000_000 {
            return Err("packet/byte count outside supported bounds");
        }
        if self.duration_ms.is_some_and(|v| v > 86_400_000) {
            return Err("duration exceeds 24 hours");
        }
        if self.protocol == Protocol::Icmp && (self.source_port != 0 || self.destination_port != 0)
        {
            return Err("ICMP ports must be zero");
        }
        if self.protocol != Protocol::Icmp && (self.source_port == 0 || self.destination_port == 0)
        {
            return Err("TCP/UDP ports must be nonzero");
        }
        if self.protocol != Protocol::Tcp && self.tcp_flags.is_some() {
            return Err("TCP flags on non-TCP event");
        }
        Ok(())
    }
}
