//! Wi-Fi Hotspot management for transferring without a shared network.
//!
//! Phase 1: isolated OS-level hotspot control via NetworkManager (`nmcli`):
//! bring a temporary access point up/down and restore the previously active
//! Wi-Fi connection. Honors a dry-run mode so development never drops the
//! dev's own connection.
//!
//! Phase 2 (`negotiation` below): build and parse the
//! `BandwidthUpgradeNegotiationFrame` used by Quick Share to switch transport
//! to `WIFI_HOTSPOT`. This is pure (de)serialization, independent of actually
//! creating an AP — so it can be unit-tested without touching the network.

use std::process::Command;

use anyhow::{Context, anyhow};

const NMCLI: &str = "nmcli";

/// True when running inside a Flatpak sandbox (where the host's `nmcli` is not
/// directly reachable and must be invoked through `flatpak-spawn --host`).
fn in_flatpak_sandbox() -> bool {
    std::path::Path::new("/.flatpak-info").exists()
}

/// Build a `Command` that runs `nmcli <args>` on the host, transparently using
/// `flatpak-spawn --host` when sandboxed. Requires the manifest to grant
/// `--talk-name=org.freedesktop.Flatpak`.
fn nmcli_command(args: &[&str]) -> Command {
    if in_flatpak_sandbox() {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg(NMCLI).args(args);
        cmd
    } else {
        let mut cmd = Command::new(NMCLI);
        cmd.args(args);
        cmd
    }
}

// Protocol types for medium-upgrade negotiation (phase 2).
use crate::location_nearby_connections::bandwidth_upgrade_negotiation_frame::{
    EventType, UpgradePathInfo,
    upgrade_path_info::{Medium, WifiHotspotCredentials},
};
use crate::location_nearby_connections::{BandwidthUpgradeNegotiationFrame, OfflineFrame, V1Frame};
use crate::location_nearby_connections::offline_frame::Version;
use crate::location_nearby_connections::v1_frame::FrameType;

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
        let out = nmcli_command(args)
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
        let out = nmcli_command(&["-t", "-f", "NAME,TYPE", "connection", "show", "--active"])
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

/// Build an `UPGRADE_PATH_AVAILABLE` offline frame offering WIFI_HOTSPOT with
/// the given credentials. This is what the host (the side that created the AP)
/// sends so the peer knows which network to join and where to reconnect.
pub fn build_hotspot_upgrade_frame(creds: &HotspotCredentials, port: i32, gateway: &str) -> OfflineFrame {
    let info = UpgradePathInfo {
        medium: Some(Medium::WifiHotspot as i32),
        wifi_hotspot_credentials: Some(WifiHotspotCredentials {
            ssid: Some(creds.ssid.clone()),
            password: Some(creds.password.clone()),
            port: Some(port),
            gateway: Some(gateway.to_string()),
            frequency: Some(-1),
        }),
        ..Default::default()
    };

    OfflineFrame {
        version: Some(Version::V1 as i32),
        v1: Some(V1Frame {
            r#type: Some(FrameType::BandwidthUpgradeNegotiation as i32),
            bandwidth_upgrade_negotiation: Some(BandwidthUpgradeNegotiationFrame {
                event_type: Some(EventType::UpgradePathAvailable as i32),
                upgrade_path_info: Some(info),
                ..Default::default()
            }),
            ..Default::default()
        }),
    }
}

/// Extract WIFI_HOTSPOT credentials from a received bandwidth-upgrade frame, if
/// it offers exactly that medium. Returns `None` for any other frame/medium.
pub fn parse_hotspot_upgrade_frame(frame: &OfflineFrame) -> Option<(HotspotCredentials, i32, String)> {
    let v1 = frame.v1.as_ref()?;
    let neg = v1.bandwidth_upgrade_negotiation.as_ref()?;
    if neg.event_type != Some(EventType::UpgradePathAvailable as i32) {
        return None;
    }
    let info = neg.upgrade_path_info.as_ref()?;
    if info.medium != Some(Medium::WifiHotspot as i32) {
        return None;
    }
    let c = info.wifi_hotspot_credentials.as_ref()?;
    let creds = HotspotCredentials {
        ssid: c.ssid.clone()?,
        password: c.password.clone().unwrap_or_default(),
        // iface is the joiner's own Wi-Fi device; filled in by the caller.
        iface: String::new(),
    };
    let port = c.port.unwrap_or(0);
    let gateway = c.gateway.clone().unwrap_or_else(|| "0.0.0.0".to_string());
    Some((creds, port, gateway))
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

    #[test]
    fn hotspot_upgrade_frame_round_trips() {
        let creds = HotspotCredentials {
            ssid: "TKShare-AB12".to_string(),
            password: "secretpass99".to_string(),
            iface: "wlp0s20f3".to_string(),
        };
        let frame = build_hotspot_upgrade_frame(&creds, 51000, "10.42.0.1");
        let (parsed, port, gateway) =
            parse_hotspot_upgrade_frame(&frame).expect("frame should parse as hotspot upgrade");
        assert_eq!(parsed.ssid, creds.ssid);
        assert_eq!(parsed.password, creds.password);
        assert_eq!(port, 51000);
        assert_eq!(gateway, "10.42.0.1");
    }

    #[test]
    fn non_hotspot_frame_is_ignored() {
        // A default/empty frame must not be mistaken for a hotspot offer.
        let frame = OfflineFrame {
            version: Some(Version::V1 as i32),
            v1: Some(V1Frame::default()),
        };
        assert!(parse_hotspot_upgrade_frame(&frame).is_none());
    }
}
