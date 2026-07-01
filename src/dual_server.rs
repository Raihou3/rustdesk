//! Dual-server (LAN + public) fallback for rendezvous and relay servers.
//!
//! ## Usage
//!
//! 1. Set config options:
//!    - `lan-rendezvous-server` — LAN ID server address (`host:port`)
//!    - `lan-relay-server` — LAN relay server address (`host:port`)
//!    - `lan-server-timeout-ms` — connection timeout for LAN attempts (default `3000`)
//!
//! 2. At startup, call [`DualServerConfig::from_config`] then
//!    [`resolve_rendezvous`] to try LAN first.
//!
//! 3. After obtaining the relay-server from the rendezvous response, call
//!    [`resolve_relay`] to substitute the LAN relay when connected via LAN.
//!
//! ## Upgrade note
//!
//! This module is designed to be replaced entirely with a newer copy.
//! Do not add crate-internal imports (e.g. `crate::xxx`) — use only
//! `hbb_common` types so the file stays portable.

use hbb_common::{
    config::{Config, RELAY_PORT, RENDEZVOUS_PORT},
    log,
    socket_client::{check_port, connect_tcp},
    timeout,
    tokio::time::Duration,
};

// ---------------------------------------------------------------------------
// Config keys
// ---------------------------------------------------------------------------

const OPTION_LAN_RENDEZVOUS_SERVER: &str = "lan-rendezvous-server";
const OPTION_LAN_RELAY_SERVER: &str = "lan-relay-server";
const OPTION_LAN_SERVER_TIMEOUT_MS: &str = "lan-server-timeout-ms";
const DEFAULT_LAN_TIMEOUT_MS: u64 = 3_000;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Runtime configuration loaded from `Config` options.
#[derive(Debug, Clone)]
pub struct DualServerConfig {
    /// LAN rendezvous server address (`host:port` or bare host).
    lan_rendezvous: Option<String>,
    /// LAN relay server address (`host:port` or bare host).
    lan_relay: Option<String>,
    /// Timeout for LAN connection attempts.
    pub timeout: Duration,
}

impl DualServerConfig {
    /// Load settings from config options, falling back to environment variables.
    pub fn from_config() -> Self {
        let rv = Config::get_option(OPTION_LAN_RENDEZVOUS_SERVER);
        let rv = if rv.is_empty() {
            std::env::var("RUSTDESK_LAN_RENDEZVOUS_SERVER").unwrap_or_default()
        } else {
            rv
        };
        let rl = Config::get_option(OPTION_LAN_RELAY_SERVER);
        let rl = if rl.is_empty() {
            std::env::var("RUSTDESK_LAN_RELAY_SERVER").unwrap_or_default()
        } else {
            rl
        };
        let ms = Config::get_option(OPTION_LAN_SERVER_TIMEOUT_MS);
        let ms = if ms.is_empty() {
            std::env::var("RUSTDESK_LAN_SERVER_TIMEOUT_MS")
                .unwrap_or_default()
                .parse::<u64>()
                .unwrap_or(DEFAULT_LAN_TIMEOUT_MS)
        } else {
            ms.parse::<u64>().unwrap_or(DEFAULT_LAN_TIMEOUT_MS)
        };
        DualServerConfig {
            lan_rendezvous: if rv.is_empty() { None } else { Some(rv) },
            lan_relay: if rl.is_empty() { None } else { Some(rl) },
            timeout: Duration::from_millis(ms.max(100)),
        }
    }

    /// Whether a LAN rendezvous server is configured.
    pub fn has_lan_rendezvous(&self) -> bool {
        self.lan_rendezvous.is_some()
    }

    /// Whether a LAN relay server is configured.
    pub fn has_lan_relay(&self) -> bool {
        self.lan_relay.is_some()
    }

    /// The configured LAN rendezvous address (without port default).
    pub fn lan_rendezvous_raw(&self) -> Option<&str> {
        self.lan_rendezvous.as_deref()
    }

    /// The configured LAN rendezvous address with default port applied.
    pub fn lan_rendezvous(&self) -> Option<String> {
        self.lan_rendezvous
            .as_ref()
            .map(|s| check_port(s, RENDEZVOUS_PORT))
    }

    /// The configured LAN relay address with default port applied.
    pub fn lan_relay(&self) -> Option<String> {
        self.lan_relay
            .as_ref()
            .map(|s| check_port(s, RELAY_PORT))
    }
}

// ---------------------------------------------------------------------------
// Rendezvous server resolution
// ---------------------------------------------------------------------------

/// Try LAN rendezvous first; fall back to the public server list.
///
/// Returns `(primary_server, fallback_servers, connected_via_lan)`.
///
/// * `public_rv` — the public server that would normally be used.
/// * `public_others` — additional public servers for fallback.
pub async fn resolve_rendezvous(
    dual: &DualServerConfig,
    public_rv: &str,
    public_others: Vec<String>,
) -> (String, Vec<String>, bool) {
    let Some(lan_rv) = dual.lan_rendezvous() else {
        // no LAN config → use public directly
        return (public_rv.to_owned(), public_others, false);
    };

    log::info!("dual_server: trying LAN rendezvous at {lan_rv}");
    match timeout(dual.timeout.as_millis() as u64, connect_tcp(&*lan_rv, dual.timeout.as_millis() as u64)).await {
        Ok(Ok(_stream)) => {
            log::info!("dual_server: connected via LAN rendezvous {lan_rv}");
            // LAN succeeded — use LAN as primary, put public in fallback
            let mut fallback = public_others;
            fallback.insert(0, check_port(public_rv, RENDEZVOUS_PORT));
            (lan_rv, fallback, true)
        }
        Ok(Err(e)) => {
            log::info!("dual_server: LAN rendezvous failed: {e}, falling back to public");
            (public_rv.to_owned(), public_others, false)
        }
        Err(_) => {
            log::info!("dual_server: LAN rendezvous timed out after {:?}, falling back to public", dual.timeout);
            (public_rv.to_owned(), public_others, false)
        }
    }
}

// ---------------------------------------------------------------------------
// Relay server resolution
// ---------------------------------------------------------------------------

/// Select the appropriate relay server based on whether we are connected via LAN.
///
/// * `connected_via_lan` — whether [`resolve_rendezvous`] returned `true`.
/// * `server_provided` — the relay address returned by the rendezvous server.
pub fn resolve_relay(dual: &DualServerConfig, connected_via_lan: bool, server_provided: &str) -> String {
    if connected_via_lan {
        if let Some(lan_relay) = dual.lan_relay() {
            log::info!("dual_server: using LAN relay {lan_relay}");
            return lan_relay;
        }
        log::info!("dual_server: connected via LAN but no LAN relay configured, using server-provided {server_provided}");
    }
    server_provided.to_owned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_empty() {
        let c = DualServerConfig::from_config();
        assert!(!c.has_lan_rendezvous());
        assert!(!c.has_lan_relay());
    }
}
