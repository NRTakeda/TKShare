//! Wi-Fi Hotspot management for transferring without a shared network.
//!
//! Phase 1 (this module): isolated OS-level hotspot control via NetworkManager
//! (`nmcli`). It does NOT touch the Quick Share protocol yet — it only knows
//! how to bring a temporary access point up and down, and to restore the
//! previously active Wi-Fi connection afterward.
//!
//! Safety: bringing up an AP takes the Wi-Fi radio off the current network, so
//! every public entry point honors a dry-run mode. With dry-run on (the
//! default), the commands are logged but not executed, which is invaluable for
//! development without dropping the developer's own connection.

use std::process::Command;

use anyhow::{Context, anyhow};

const NMCLI: &str = "nmcli";

/// Credentials for a hotspot we created (to hand to the peer) or that we were
/// told to join.
#[derive(Debug, Clone)]
pub struct HotspotCredentials {
    pub ssid: String,
    pub password: String,
    /// Wi-Fi interface backing the hotspot (e.g. "wlp0s20f3").
    pub iface: String,
}

/// Manages a temporary hotspot lifecycle and remembers what to restore.
#[derive(Debug)]
pub struct HotspotManager {
    /// When true, commands are only logged, never executed.
    dry_run: bool,
    /// Name of the Wi-Fi connection that was active before we started, so we
    /// can bring it back up on teardown.
    previous_connection: Option<String>,
    /// Name of the NetworkManager connection we created for the hotspot.
    created_connection: Option<String>,
}

impl HotspotManager {
    pub fn new(dry_run: bool) -> Self {
        Self {
            dry_run,
            previous_connection: None,
            created_connection: None,
        }
    }

    /// Run an nmcli invocation, or just log it in dry-run mode.
    fn nmcli(&self, args: &[&str]) -> anyhow::Result<String> {
        if self.dry_run {
            info!("[hotspot dry-run] {NMCLI} {}", args.join(" "));
            return Ok(String::new());
        }
        let out = Command::new(NMCLI)
            .args(args)
            .output()
            .with_context(|| format!("failed to spawn {NMCLI}"))?;
        if !out.status.success() {
            return Err(anyhow!(
                "{NMCLI} {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    }

    /// Returns the currently active Wi-Fi connection name, if any.
    fn active_wifi_connection(&self) -> anyhow::Result<Option<String>> {
        // In dry-run we can still query (read-only), so don't short-circuit.
        let out = Command::new(NMCLI)
            .args(["-t", "-f", "NAME,TYPE", "connection", "show", "--active"])
            .output()
            .with_context(|| format!("failed to spawn {NMCLI}"))?;
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            // Format: NAME:TYPE  (TYPE is like 802-11-wireless)
            if let Some((name, ty)) = line.rsplit_once(':') {
                if ty.contains("wireless") {
                    return Ok(Some(name.to_string()));
                }
            }
        }
        Ok(None)
    }

    /// Create a temporary hotspot. Remembers the previously active Wi-Fi
    /// connection so [`teardown`] can restore it.
    pub fn create(&mut self, iface: &str) -> anyhow::Result<HotspotCredentials> {
        self.previous_connection = self.active_wifi_connection().ok().flatten();
        if let Some(prev) = &self.previous_connection {
            info!("hotspot: will restore connection '{prev}' on teardown");
        }

        let ssid = format!("TKShare-{}", random_suffix(4));
        let password = random_password(12);
        let con_name = format!("tkshare-hotspot-{}", random_suffix(4));

        // `nmcli device wifi hotspot` is the simplest path; it creates and
        // activates an AP connection in one go.
        self.nmcli(&[
            "device",
            "wifi",
            "hotspot",
            "ifname",
            iface,
            "con-name",
            &con_name,
            "ssid",
            &ssid,
            "password",
            &password,
        ])?;
        self.created_connection = Some(con_name);

        Ok(HotspotCredentials {
            ssid,
            password,
            iface: iface.to_string(),
        })
    }

    /// Join a hotspot advertised by the peer.
    pub fn connect(&self, creds: &HotspotCredentials) -> anyhow::Result<()> {
        self.nmcli(&[
            "device",
            "wifi",
            "connect",
            &creds.ssid,
            "password",
            &creds.password,
            "ifname",
            &creds.iface,
        ])?;
        Ok(())
    }

    /// Tear down whatever we created and restore the previous connection.
    pub fn teardown(&mut self) -> anyhow::Result<()> {
        if let Some(con) = self.created_connection.take() {
            // `down` then `delete` so we don't leave a stale profile behind.
            let _ = self.nmcli(&["connection", "down", &con]);
            let _ = self.nmcli(&["connection", "delete", &con]);
        }
        if let Some(prev) = self.previous_connection.take() {
            info!("hotspot: restoring connection '{prev}'");
            let _ = self.nmcli(&["connection", "up", &prev]);
        }
        Ok(())
    }
}

impl Drop for HotspotManager {
    fn drop(&mut self) {
        // Best-effort cleanup so a dropped manager never leaves an AP up.
        let _ = self.teardown();
    }
}

fn random_suffix(len: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())] as char)
        .collect()
}

fn random_password(len: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijkmnpqrstuvwxyz23456789";
    let mut rng = rand::rng();
    (0..len)
        .map(|_| CHARS[rng.random_range(0..CHARS.len())] as char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dry_run_create_and_teardown_do_not_execute() {
        // With dry-run on, the full lifecycle must succeed without touching the
        // network (commands are only logged).
        let mut mgr = HotspotManager::new(true);
        let creds = mgr.create("wlan-test").expect("create (dry-run) ok");
        assert!(creds.ssid.starts_with("TKShare-"));
        assert_eq!(creds.password.len(), 12);
        assert_eq!(creds.iface, "wlan-test");
        mgr.connect(&creds).expect("connect (dry-run) ok");
        mgr.teardown().expect("teardown (dry-run) ok");
    }

    #[test]
    fn generated_credentials_are_random() {
        assert_ne!(random_suffix(8), random_suffix(8));
        assert_ne!(random_password(16), random_password(16));
    }
}
