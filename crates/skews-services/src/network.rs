//! Wi-Fi status and control through NetworkManager's D-Bus API.
//!
//! This adapter talks the protocol directly (ADR-0005): typed zbus blocking
//! proxies, no CLI parsing and no shell. The [`NetworkSource`] trait keeps the
//! modules testable with fakes.

use thiserror::Error;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

/// Errors produced by the network adapter.
#[derive(Debug, Error)]
pub enum NetworkError {
    /// The D-Bus call failed.
    #[error("network manager call failed: {0}")]
    Dbus(#[from] zbus::Error),
    /// A property call failed.
    #[error("network manager property call failed: {0}")]
    Property(#[from] zbus::fdo::Error),
    /// No Wi-Fi device is available.
    #[error("no Wi-Fi device found")]
    NoWifiDevice,
    /// The requested network does not exist in the scan results.
    #[error("network `{0}` not found")]
    NotFound(String),
    /// The network requires a password (secret-agent flow is not implemented).
    #[error("network `{0}` requires a password")]
    Secured(String),
}

/// NetworkManager connectivity state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Connectivity {
    /// Unknown (no answer yet).
    Unknown,
    /// No connectivity.
    None,
    /// Behind a captive portal.
    Portal,
    /// Limited connectivity.
    Limited,
    /// Full internet access.
    Full,
}

impl Connectivity {
    fn from_raw(raw: u32) -> Self {
        match raw {
            1 => Self::None,
            2 => Self::Portal,
            3 => Self::Limited,
            4 => Self::Full,
            _ => Self::Unknown,
        }
    }
}

/// Wi-Fi status snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkStatus {
    /// Whether the Wi-Fi radio is enabled.
    pub wifi_enabled: bool,
    /// SSID of the active connection, when connected over Wi-Fi.
    pub ssid: Option<String>,
    /// Signal strength (0-100) of the active connection.
    pub signal: Option<u8>,
    /// Connectivity state.
    pub connectivity: Connectivity,
}

/// One visible access point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessPoint {
    /// Network name.
    pub ssid: String,
    /// Signal strength (0-100).
    pub signal: u8,
    /// Whether the network is secured.
    pub security: bool,
    /// Whether this is the active connection.
    pub active: bool,
}

/// Port for Wi-Fi status and control (real: NetworkManager; tests: fakes).
pub trait NetworkSource: Send + Sync {
    /// Reads the current status.
    fn status(&self) -> Result<NetworkStatus, NetworkError>;
    /// Lists visible access points.
    fn access_points(&self) -> Result<Vec<AccessPoint>, NetworkError>;
    /// Enables or disables the Wi-Fi radio.
    fn set_wifi_enabled(&self, enabled: bool) -> Result<(), NetworkError>;
    /// Connects to a network by SSID (open networks only for now).
    fn connect(&self, ssid: &str) -> Result<(), NetworkError>;
}

/// NetworkManager-backed source.
pub struct NetworkManager;

const NM_SERVICE: &str = "org.freedesktop.NetworkManager";
const NM_PATH: &str = "/org/freedesktop/NetworkManager";
const DEVICE_TYPE_WIFI: u32 = 2;
/// NetworkManager `NM_802_11_AP_FLAGS_PRIVACY`.
const AP_FLAG_PRIVACY: u32 = 0x1;

impl NetworkManager {
    fn connection() -> Result<Connection, NetworkError> {
        Ok(Connection::system()?)
    }

    fn wifi_device(conn: &Connection) -> Result<OwnedObjectPath, NetworkError> {
        let manager = Proxy::new(conn, NM_SERVICE, NM_PATH, "org.freedesktop.NetworkManager")?;
        let devices: Vec<OwnedObjectPath> = manager.call("GetDevices", &())?;

        for device in devices {
            let path = device.as_str().to_owned();
            let device_type: u32 = {
                let proxy = Proxy::new(
                    conn,
                    NM_SERVICE,
                    path,
                    "org.freedesktop.NetworkManager.Device",
                )?;
                proxy.get_property("DeviceType")?
            };
            if device_type == DEVICE_TYPE_WIFI {
                return Ok(device);
            }
        }

        Err(NetworkError::NoWifiDevice)
    }

    fn wireless<'a>(
        conn: &'a Connection,
        device: &OwnedObjectPath,
    ) -> Result<Proxy<'a>, NetworkError> {
        Ok(Proxy::new(
            conn,
            NM_SERVICE,
            device.as_str().to_owned(),
            "org.freedesktop.NetworkManager.Device.Wireless",
        )?)
    }

    fn access_point<'a>(
        conn: &'a Connection,
        path: &OwnedObjectPath,
    ) -> Result<Proxy<'a>, NetworkError> {
        Ok(Proxy::new(
            conn,
            NM_SERVICE,
            path.as_str().to_owned(),
            "org.freedesktop.NetworkManager.AccessPoint",
        )?)
    }

    fn ssid_of(proxy: &Proxy<'_>) -> Result<String, NetworkError> {
        let bytes: Vec<u8> = proxy.get_property("Ssid")?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    fn active_ssid(
        conn: &Connection,
        device: &OwnedObjectPath,
    ) -> Result<Option<(String, u8)>, NetworkError> {
        let wireless = Self::wireless(conn, device)?;
        let active: OwnedObjectPath = wireless.get_property("ActiveAccessPoint")?;
        if active.as_str() == "/" {
            return Ok(None);
        }

        let ap = Self::access_point(conn, &active)?;
        let ssid = Self::ssid_of(&ap)?;
        let signal: u8 = ap.get_property("Strength")?;

        Ok(Some((ssid, signal)))
    }
}

impl NetworkSource for NetworkManager {
    fn status(&self) -> Result<NetworkStatus, NetworkError> {
        let conn = Self::connection()?;
        let manager = Proxy::new(&conn, NM_SERVICE, NM_PATH, "org.freedesktop.NetworkManager")?;

        let wifi_enabled: bool = manager.get_property("WirelessEnabled")?;
        let connectivity_raw: u32 = manager.get_property("Connectivity")?;

        let active = match Self::wifi_device(&conn) {
            Ok(device) if wifi_enabled => Self::active_ssid(&conn, &device)?,
            _ => None,
        };
        let (ssid, signal) = match active {
            Some((ssid, signal)) => (Some(ssid), Some(signal)),
            None => (None, None),
        };

        Ok(NetworkStatus {
            wifi_enabled,
            ssid,
            signal,
            connectivity: Connectivity::from_raw(connectivity_raw),
        })
    }

    fn access_points(&self) -> Result<Vec<AccessPoint>, NetworkError> {
        let conn = Self::connection()?;
        let device = Self::wifi_device(&conn)?;
        let wireless = Self::wireless(&conn, &device)?;

        let active: OwnedObjectPath = wireless.get_property("ActiveAccessPoint")?;
        let paths: Vec<OwnedObjectPath> = wireless.call("GetAccessPoints", &())?;

        let mut points = Vec::new();
        for path in paths {
            let ap = Self::access_point(&conn, &path)?;
            let ssid = Self::ssid_of(&ap)?;
            if ssid.is_empty() {
                continue;
            }

            let signal: u8 = ap.get_property("Strength")?;
            let flags: u32 = ap.get_property("Flags")?;
            let wpa_flags: u32 = ap.get_property("WpaFlags")?;

            points.push(AccessPoint {
                ssid,
                signal,
                security: flags & AP_FLAG_PRIVACY != 0 || wpa_flags != 0,
                active: path.as_str() == active.as_str(),
            });
        }

        points.sort_by(|a, b| b.signal.cmp(&a.signal).then_with(|| a.ssid.cmp(&b.ssid)));
        Ok(points)
    }

    fn set_wifi_enabled(&self, enabled: bool) -> Result<(), NetworkError> {
        let conn = Self::connection()?;
        let manager = Proxy::new(&conn, NM_SERVICE, NM_PATH, "org.freedesktop.NetworkManager")?;
        manager.set_property("WirelessEnabled", enabled)?;
        Ok(())
    }

    fn connect(&self, ssid: &str) -> Result<(), NetworkError> {
        let conn = Self::connection()?;
        let device = Self::wifi_device(&conn)?;
        let wireless = Self::wireless(&conn, &device)?;
        let paths: Vec<OwnedObjectPath> = wireless.call("GetAccessPoints", &())?;

        let mut target = None;
        for path in paths {
            let ap = Self::access_point(&conn, &path)?;
            if Self::ssid_of(&ap)? == ssid {
                let flags: u32 = ap.get_property("Flags")?;
                let wpa_flags: u32 = ap.get_property("WpaFlags")?;
                if flags & AP_FLAG_PRIVACY != 0 || wpa_flags != 0 {
                    return Err(NetworkError::Secured(ssid.to_owned()));
                }
                target = Some(path);
                break;
            }
        }

        let Some(ap_path) = target else {
            return Err(NetworkError::NotFound(ssid.to_owned()));
        };

        let manager = Proxy::new(&conn, NM_SERVICE, NM_PATH, "org.freedesktop.NetworkManager")?;
        let _: OwnedObjectPath = manager.call(
            "ActivateConnection",
            &(
                OwnedObjectPath::try_from("/").expect("root path"),
                device,
                ap_path,
            ),
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Connectivity;

    #[test]
    fn maps_connectivity_values() {
        assert_eq!(Connectivity::from_raw(0), Connectivity::Unknown);
        assert_eq!(Connectivity::from_raw(1), Connectivity::None);
        assert_eq!(Connectivity::from_raw(2), Connectivity::Portal);
        assert_eq!(Connectivity::from_raw(3), Connectivity::Limited);
        assert_eq!(Connectivity::from_raw(4), Connectivity::Full);
        assert_eq!(Connectivity::from_raw(99), Connectivity::Unknown);
    }
}
