#[cfg(target_os = "linux")]
use crate::device_info::{DeviceEventType, DeviceHandle, UsbDeviceInfo};
#[cfg(target_os = "linux")]
use std::collections::HashMap;
#[cfg(target_os = "linux")]
use std::path::Path;
#[cfg(target_os = "linux")]
use tokio::fs;
#[cfg(target_os = "linux")]
use tokio::sync::mpsc;

#[cfg(target_os = "linux")]
/// Linux-specific USB device watcher implementation.
///
/// This watcher uses the Linux sysfs filesystem (`/sys/bus/usb/devices`) to monitor
/// USB device connections and disconnections. It polls the filesystem periodically
/// to detect changes and sends events through the provided channel.
///
/// Device handles are provided for each detected device, including sysfs path.
/// Future versions may detect device nodes (e.g., `/dev/ttyUSB0`).
pub struct LinuxUsbWatcher {
    tx: mpsc::Sender<UsbDeviceInfo>,
}

#[cfg(target_os = "linux")]
impl LinuxUsbWatcher {
    /// Creates a new Linux USB watcher.
    ///
    /// # Arguments
    ///
    /// * `tx` - Channel sender for broadcasting USB device events
    ///
    /// # Returns
    ///
    /// A new `LinuxUsbWatcher` instance
    pub fn new(tx: mpsc::Sender<UsbDeviceInfo>) -> Self {
        Self { tx }
    }

    /// Starts monitoring USB devices on Linux.
    ///
    /// This method continuously polls the `/sys/bus/usb/devices` directory
    /// to detect USB device connection and disconnection events. It maintains
    /// a map of known devices and compares against current devices to identify
    /// changes.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if monitoring stops gracefully, or `Err(String)` if
    /// an error occurs during monitoring.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The USB devices directory is not accessible
    /// - File system operations fail
    /// - Device information parsing fails
    pub async fn start_monitoring(&self) -> Result<(), String> {
        println!("Starting USB device monitoring on Linux...");

        // Simple polling approach - check /sys/bus/usb/devices periodically
        let mut known_devices: HashMap<String, UsbDeviceInfo> = HashMap::new();
        let mut poll_interval = tokio::time::Duration::from_secs(1); // Start with 1s, adapt based on activity

        loop {
            let scan_start = std::time::Instant::now();

            match self.scan_usb_devices().await {
                Ok(current_devices) => {
                    let current_map: HashMap<String, UsbDeviceInfo> = current_devices
                        .into_iter()
                        .map(|d| {
                            let key = format!(
                                "{}:{}:{}",
                                d.vendor_id,
                                d.product_id,
                                d.serial_number.as_deref().unwrap_or("unknown")
                            );
                            (key, d)
                        })
                        .collect();

                    let mut events_sent = 0;

                    // Check for new devices (connected)
                    for (key, device) in &current_map {
                        if !known_devices.contains_key(key) {
                            let mut device_clone = device.clone();
                            device_clone.event_type = DeviceEventType::Connected;
                            if let Err(e) = self.tx.send(device_clone).await {
                                eprintln!("Failed to send device event: {e}");
                            } else {
                                events_sent += 1;
                            }
                        }
                    }

                    // Check for removed devices (disconnected)
                    for (key, device) in &known_devices {
                        if !current_map.contains_key(key) {
                            let mut device_clone = device.clone();
                            device_clone.event_type = DeviceEventType::Disconnected;
                            if let Err(e) = self.tx.send(device_clone).await {
                                eprintln!("Failed to send device event: {e}");
                            } else {
                                events_sent += 1;
                            }
                        }
                    }

                    known_devices = current_map;

                    // Adaptive polling: reduce interval if there's activity, increase if idle
                    if events_sent > 0 {
                        poll_interval = tokio::time::Duration::from_millis(500);
                    // Fast polling when active
                    } else {
                        poll_interval = std::cmp::min(
                            poll_interval + tokio::time::Duration::from_millis(100),
                            tokio::time::Duration::from_secs(5), // Max 5 seconds when idle
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Error scanning USB devices: {e}");
                    // Use exponential backoff on errors
                    poll_interval =
                        std::cmp::min(poll_interval * 2, tokio::time::Duration::from_secs(10));
                }
            }

            // Ensure we don't scan too frequently
            let scan_duration = scan_start.elapsed();
            if scan_duration < poll_interval {
                tokio::time::sleep(poll_interval - scan_duration).await;
            }
        }
    }

    async fn scan_usb_devices(&self) -> Result<Vec<UsbDeviceInfo>, String> {
        let mut devices = Vec::new();
        let usb_devices_path = Path::new("/sys/bus/usb/devices");

        if !usb_devices_path.exists() {
            return Err(
                "USB devices path not found. Make sure you're running on Linux with USB support."
                    .to_string(),
            );
        }

        let mut entries = fs::read_dir(usb_devices_path)
            .await
            .map_err(|e| e.to_string())?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
            let path = entry.path();

            // Skip entries that don't look like USB devices (e.g., usb1, usb2, etc.)
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                // Look for actual USB devices and root hubs, but skip interfaces
                // USB devices: patterns like "1-1", "1-2", "2-1", etc. (devices connected to ports)
                // USB root hubs: "usb1", "usb2", etc.
                // Skip interfaces: "1-0:1.0", "2-0:1.0", etc.
                let is_device = (name.matches('-').count() == 1 && !name.contains(':'))
                    || name.starts_with("usb");
                let is_interface = name.contains(':');

                if is_device && !is_interface {
                    if let Ok(device_info) = self.parse_usb_device(&path).await {
                        // Skip devices with all zero VID/PID (typically means no actual device info)
                        if device_info.vendor_id != "0000" || device_info.product_id != "0000" {
                            devices.push(device_info);
                        }
                    } else {
                        println!("Failed to parse device: {name}");
                    }
                }
            }
        }

        Ok(devices)
    }

    async fn parse_usb_device(&self, device_path: &Path) -> Result<UsbDeviceInfo, String> {
        let vendor_id = self
            .read_sys_file(device_path, "idVendor")
            .await
            .unwrap_or_else(|| "0000".to_string());
        let product_id = self
            .read_sys_file(device_path, "idProduct")
            .await
            .unwrap_or_else(|| "0000".to_string());

        let product_name = self
            .read_sys_file(device_path, "product")
            .await
            .unwrap_or_else(|| "Unknown Device".to_string());
        let manufacturer = self
            .read_sys_file(device_path, "manufacturer")
            .await
            .unwrap_or_default();
        let serial_number = self.read_sys_file(device_path, "serial").await;

        let device_name = if !manufacturer.is_empty() && !product_name.is_empty() {
            format!("{manufacturer} {product_name}")
        } else if !product_name.is_empty() {
            product_name
        } else {
            "Unknown Device".to_string()
        };

        let device_handle = DeviceHandle::Linux {
            sysfs_path: device_path.to_string_lossy().to_string(),
            device_node: None, // Could be enhanced to detect device nodes
        };

        Ok(UsbDeviceInfo::with_handle(
            device_name,
            vendor_id,
            product_id,
            serial_number,
            DeviceEventType::Connected, // Will be updated by caller
            device_handle,
        ))
    }
    async fn read_sys_file(&self, device_path: &Path, filename: &str) -> Option<String> {
        let file_path = device_path.join(filename);
        fs::read_to_string(file_path)
            .await
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }
}

#[cfg(not(target_os = "linux"))]
pub struct LinuxUsbWatcher;

#[cfg(not(target_os = "linux"))]
impl LinuxUsbWatcher {
    pub fn new(_tx: tokio::sync::mpsc::Sender<crate::device_info::UsbDeviceInfo>) -> Self {
        Self
    }

    pub async fn start_monitoring(&self) -> Result<(), String> {
        Err("Linux USB monitoring not available on this platform".to_string())
    }
}
