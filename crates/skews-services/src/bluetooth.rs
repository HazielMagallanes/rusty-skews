//! Bluetooth status and control through BlueZ's D-Bus API.
//!
//! Uses the ObjectManager to enumerate adapters and devices, then typed
//! property reads on per-object proxies (ADR-0005: protocol, not CLI). The
//! [`BluetoothSource`] trait keeps modules testable with fakes.

use std::collections::HashMap;

use thiserror::Error;
use zbus::blocking::{Connection, Proxy};
use zbus::zvariant::OwnedObjectPath;

/// Errors produced by the Bluetooth adapter.
#[derive(Debug, Error)]
pub enum BluetoothError {
    /// The D-Bus call failed.
    #[error("bluez call failed: {0}")]
    Dbus(#[from] zbus::Error),
    /// A property call failed.
    #[error("bluez property call failed: {0}")]
    Property(#[from] zbus::fdo::Error),
    /// No Bluetooth adapter is present.
    #[error("no Bluetooth adapter found")]
    NoAdapter,
    /// The requested device does not exist.
    #[error("Bluetooth device `{0}` not found")]
    DeviceNotFound(String),
}

/// Adapter status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterStatus {
    /// Whether the adapter is powered.
    pub powered: bool,
    /// Adapter name (e.g. the machine name).
    pub name: String,
}

/// One known Bluetooth device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BluetoothDevice {
    /// MAC address (stable identifier for actions).
    pub address: String,
    /// Display name (alias when set).
    pub name: String,
    /// Whether the device is connected right now.
    pub connected: bool,
    /// Whether the device is paired.
    pub paired: bool,
}

/// Port for Bluetooth status and control.
pub trait BluetoothSource: Send + Sync {
    /// Reads adapter status.
    fn status(&self) -> Result<AdapterStatus, BluetoothError>;
    /// Lists known devices.
    fn devices(&self) -> Result<Vec<BluetoothDevice>, BluetoothError>;
    /// Powers the adapter on or off.
    fn set_powered(&self, powered: bool) -> Result<(), BluetoothError>;
    /// Connects a device by address.
    fn connect(&self, address: &str) -> Result<(), BluetoothError>;
    /// Disconnects a device by address.
    fn disconnect(&self, address: &str) -> Result<(), BluetoothError>;
}

/// BlueZ-backed source.
pub struct Bluez;

const BLUEZ_SERVICE: &str = "org.bluez";
const ADAPTER_INTERFACE: &str = "org.bluez.Adapter1";
const DEVICE_INTERFACE: &str = "org.bluez.Device1";

type ManagedObjects =
    HashMap<OwnedObjectPath, HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>>;

impl Bluez {
    fn connection() -> Result<Connection, BluetoothError> {
        Ok(Connection::system()?)
    }

    fn managed_objects(conn: &Connection) -> Result<ManagedObjects, BluetoothError> {
        let reply = conn.call_method(
            Some(BLUEZ_SERVICE),
            "/",
            Some("org.freedesktop.DBus.ObjectManager"),
            "GetManagedObjects",
            &(),
        )?;
        Ok(reply.body().deserialize()?)
    }

    fn adapter_path(conn: &Connection) -> Result<OwnedObjectPath, BluetoothError> {
        let objects = Self::managed_objects(conn)?;
        objects
            .into_iter()
            .find(|(_, interfaces)| interfaces.contains_key(ADAPTER_INTERFACE))
            .map(|(path, _)| path)
            .ok_or(BluetoothError::NoAdapter)
    }

    fn device_path(conn: &Connection, address: &str) -> Result<OwnedObjectPath, BluetoothError> {
        let objects = Self::managed_objects(conn)?;

        for (path, interfaces) in objects {
            if !interfaces.contains_key(DEVICE_INTERFACE) {
                continue;
            }
            let proxy = Proxy::new(
                conn,
                BLUEZ_SERVICE,
                path.as_str().to_owned(),
                DEVICE_INTERFACE,
            )?;
            let found: String = proxy.get_property("Address")?;
            if found.eq_ignore_ascii_case(address) {
                return Ok(path);
            }
        }

        Err(BluetoothError::DeviceNotFound(address.to_owned()))
    }

    fn device_proxy<'a>(conn: &'a Connection, address: &str) -> Result<Proxy<'a>, BluetoothError> {
        let path = Self::device_path(conn, address)?;
        Ok(Proxy::new(
            conn,
            BLUEZ_SERVICE,
            path.as_str().to_owned(),
            DEVICE_INTERFACE,
        )?)
    }
}

impl BluetoothSource for Bluez {
    fn status(&self) -> Result<AdapterStatus, BluetoothError> {
        let conn = Self::connection()?;
        let path = Self::adapter_path(&conn)?;
        let proxy = Proxy::new(
            &conn,
            BLUEZ_SERVICE,
            path.as_str().to_owned(),
            ADAPTER_INTERFACE,
        )?;

        Ok(AdapterStatus {
            powered: proxy.get_property("Powered")?,
            name: proxy.get_property::<String>("Alias").unwrap_or_default(),
        })
    }

    fn devices(&self) -> Result<Vec<BluetoothDevice>, BluetoothError> {
        let conn = Self::connection()?;
        let objects = Self::managed_objects(&conn)?;

        let mut devices = Vec::new();
        for (path, interfaces) in objects {
            if !interfaces.contains_key(DEVICE_INTERFACE) {
                continue;
            }

            let proxy = Proxy::new(
                &conn,
                BLUEZ_SERVICE,
                path.as_str().to_owned(),
                DEVICE_INTERFACE,
            )?;
            let address: String = proxy.get_property("Address")?;
            let alias: String = proxy
                .get_property::<String>("Alias")
                .or_else(|_| proxy.get_property::<String>("Name"))
                .unwrap_or_else(|_| address.clone());
            let connected: bool = proxy.get_property("Connected")?;
            let paired: bool = proxy.get_property("Paired")?;

            devices.push(BluetoothDevice {
                address,
                name: alias,
                connected,
                paired,
            });
        }

        devices.sort_by(|a, b| {
            b.connected
                .cmp(&a.connected)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(devices)
    }

    fn set_powered(&self, powered: bool) -> Result<(), BluetoothError> {
        let conn = Self::connection()?;
        let path = Self::adapter_path(&conn)?;
        let proxy = Proxy::new(
            &conn,
            BLUEZ_SERVICE,
            path.as_str().to_owned(),
            ADAPTER_INTERFACE,
        )?;
        proxy.set_property("Powered", powered)?;
        Ok(())
    }

    fn connect(&self, address: &str) -> Result<(), BluetoothError> {
        let conn = Self::connection()?;
        let proxy = Self::device_proxy(&conn, address)?;
        proxy.call::<_, _, ()>("Connect", &())?;
        Ok(())
    }

    fn disconnect(&self, address: &str) -> Result<(), BluetoothError> {
        let conn = Self::connection()?;
        let proxy = Self::device_proxy(&conn, address)?;
        proxy.call::<_, _, ()>("Disconnect", &())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // The BlueZ adapter is thin glue over typed D-Bus calls; its behavior is
    // covered by live verification and by the module tests with fakes.
}
