//!
//! Windows-specific USB device watcher implementation.
//!
//! This implementation uses Unicode (UTF-16) Windows APIs to properly handle
//! international characters in device names and descriptions. This ensures
//! devices with non-ASCII names (Chinese, Japanese, accented characters, etc.)
//! are displayed correctly instead of showing garbled text.
//!
#[cfg(target_os = "windows")]
use crate::device_info::{DeviceEventType, DeviceHandle, UsbDeviceInfo};
#[cfg(target_os = "windows")]
use std::collections::HashSet;
#[cfg(target_os = "windows")]
use tokio::sync::mpsc;
#[cfg(target_os = "windows")]
use windows::{core::*, Win32::Devices::DeviceAndDriverInstallation::*, Win32::Foundation::PWSTR};

#[cfg(target_os = "windows")]
pub struct WindowsUsbWatcher {
    tx: mpsc::Sender<UsbDeviceInfo>,
}

#[cfg(target_os = "windows")]
impl WindowsUsbWatcher {
    pub fn new(tx: mpsc::Sender<UsbDeviceInfo>) -> Self {
        Self { tx }
    }

    pub async fn start_monitoring(&self) -> std::result::Result<(), String> {
        println!("Starting USB device monitoring on Windows...");

        // For this implementation, we'll use a simple polling approach
        // In a production environment, you'd want to use proper Windows notifications
        let mut known_devices = HashSet::new();

        loop {
            match self.scan_usb_devices().await {
                Ok(current_devices) => {
                    // Check for new devices (connected)
                    for device in &current_devices {
                        let device_key = format!("{}:{}", device.vendor_id, device.product_id);
                        if !known_devices.contains(&device_key) {
                            known_devices.insert(device_key.clone());
                            let mut device_clone = device.clone();
                            device_clone.event_type = DeviceEventType::Connected;
                            if let Err(e) = self.tx.send(device_clone).await {
                                eprintln!("Failed to send device event: {}", e);
                            }
                        }
                    }

                    // Check for removed devices (disconnected)
                    let current_keys: HashSet<String> = current_devices
                        .iter()
                        .map(|d| format!("{}:{}", d.vendor_id, d.product_id))
                        .collect();

                    let removed_keys: Vec<String> =
                        known_devices.difference(&current_keys).cloned().collect();

                    for key in removed_keys {
                        known_devices.remove(&key);
                        let parts: Vec<&str> = key.split(':').collect();
                        if parts.len() == 2 {
                            let device_info = UsbDeviceInfo::new(
                                "Unknown Device".to_string(),
                                parts[0].to_string(),
                                parts[1].to_string(),
                                None,
                                DeviceEventType::Disconnected,
                            );
                            if let Err(e) = self.tx.send(device_info).await {
                                eprintln!("Failed to send device event: {}", e);
                            }
                        }
                    }
                }
                Err(e) => {
                    eprintln!("Error scanning USB devices: {}", e);
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    }

    async fn scan_usb_devices(&self) -> std::result::Result<Vec<UsbDeviceInfo>, String> {
        let mut devices = Vec::new();

        unsafe {
            // Get USB device class GUID
            let mut class_guid_buffer = [GUID::default(); 1];
            let mut required_size = 0u32;
            if SetupDiClassGuidsFromNameW(
                &HSTRING::from("USB"),
                &mut class_guid_buffer,
                &mut required_size,
            )
            .is_err()
            {
                return Err("Failed to get USB class GUID".to_string());
            }
            let class_guid = class_guid_buffer[0];

            // Get device information set
            let device_info_set =
                SetupDiGetClassDevsW(Some(&class_guid), PCWSTR::null(), None, DIGCF_PRESENT)
                    .map_err(|e| format!("Failed to get device info set: {}", e))?;

            if device_info_set.is_invalid() {
                return Err("Failed to get device information set".to_string());
            }

            let mut device_index = 0u32;
            let mut device_info_data = SP_DEVINFO_DATA {
                cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32,
                ..Default::default()
            };

            while SetupDiEnumDeviceInfo(device_info_set, device_index, &mut device_info_data)
                .is_ok()
            {
                if let Ok(device_info) = self
                    .get_device_info(device_info_set, &device_info_data)
                    .await
                {
                    devices.push(device_info);
                }
                device_index += 1;
            }

            SetupDiDestroyDeviceInfoList(device_info_set)
                .map_err(|e| format!("Failed to destroy device info list: {}", e))?;
        }

        Ok(devices)
    }

    async fn get_device_info(
        &self,
        device_info_set: HDEVINFO,
        device_info_data: &SP_DEVINFO_DATA,
    ) -> std::result::Result<UsbDeviceInfo, String> {
        // Get device description
        let device_name = self
            .get_device_property(device_info_set, device_info_data, SPDRP_DEVICEDESC)
            .unwrap_or_else(|| "Unknown Device".to_string());

        // Get hardware ID to extract VID/PID
        let hardware_id = self
            .get_device_property(device_info_set, device_info_data, SPDRP_HARDWAREID)
            .unwrap_or_default();

        let (vendor_id, product_id) = self.parse_vid_pid(&hardware_id);

        // Try to get serial number
        let serial_number = self.get_device_property(
            device_info_set,
            device_info_data,
            SPDRP_PHYSICAL_DEVICE_OBJECT_NAME,
        );

        Ok(UsbDeviceInfo::new(
            device_name,
            vendor_id,
            product_id,
            serial_number,
            DeviceEventType::Connected, // Will be updated by caller
        ))
    }

    /// Gets a device property from the Windows registry.
    ///
    /// Uses the Unicode (UTF-16) Windows API to properly handle international
    /// characters in device names and descriptions. This is important for
    /// devices with non-ASCII names (e.g., Chinese, Japanese, or accented characters).
    fn get_device_property(
        &self,
        device_info_set: HDEVINFO,
        device_info_data: &SP_DEVINFO_DATA,
        property: SETUP_DI_REGISTRY_PROPERTY,
    ) -> Option<String> {
        unsafe {
            let mut required_size = 0u32;
            let mut property_type = 0u32;

            // Get required buffer size using Unicode API
            let _ = SetupDiGetDeviceRegistryPropertyW(
                device_info_set,
                device_info_data,
                property,
                Some(&mut property_type),
                None,
                Some(&mut required_size),
            );

            if required_size == 0 {
                return None;
            }

            // Allocate buffer for UTF-16 data (required_size is in bytes, so divide by 2 for u16 count)
            let utf16_len = (required_size as usize + 1) / 2; // +1 to handle odd byte counts
            let mut utf16_buffer = vec![0u16; utf16_len];
            let buffer_bytes = std::slice::from_raw_parts_mut(
                utf16_buffer.as_mut_ptr() as *mut u8,
                required_size as usize,
            );

            if SetupDiGetDeviceRegistryPropertyW(
                device_info_set,
                device_info_data,
                property,
                Some(&mut property_type),
                Some(buffer_bytes),
                Some(&mut required_size),
            )
            .is_ok()
            {
                // Find the actual end of the string (before null terminator)
                let actual_len = utf16_buffer
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(utf16_buffer.len());

                // Convert UTF-16 to String, handling international characters properly
                let result = String::from_utf16_lossy(&utf16_buffer[..actual_len]);

                if !result.is_empty() {
                    Some(result)
                } else {
                    None
                }
            } else {
                None
            }
        }
    }

    fn parse_vid_pid(&self, hardware_id: &str) -> (String, String) {
        // Parse hardware ID like "USB\VID_046D&PID_C52B&REV_1200"
        let mut vendor_id = "0000".to_string();
        let mut product_id = "0000".to_string();

        // Find VID
        if let Some(vid_start) = hardware_id.find("VID_") {
            let vid_str = &hardware_id[vid_start + 4..];
            if let Some(vid_end) = vid_str.find('&').or_else(|| vid_str.find('\0')) {
                if vid_end >= 4 {
                    vendor_id = vid_str[..4].to_string();
                }
            } else if vid_str.len() >= 4 {
                vendor_id = vid_str[..4].to_string();
            }
        }

        // Find PID
        if let Some(pid_start) = hardware_id.find("PID_") {
            let pid_str = &hardware_id[pid_start + 4..];
            if let Some(pid_end) = pid_str.find('&').or_else(|| pid_str.find('\0')) {
                if pid_end >= 4 {
                    product_id = pid_str[..4].to_string();
                }
            } else if pid_str.len() >= 4 {
                product_id = pid_str[..4].to_string();
            }
        }

        (vendor_id, product_id)
    }
}

#[cfg(not(target_os = "windows"))]
pub struct WindowsUsbWatcher;

#[cfg(not(target_os = "windows"))]
impl WindowsUsbWatcher {
    pub fn new(_tx: tokio::sync::mpsc::Sender<crate::device_info::UsbDeviceInfo>) -> Self {
        Self
    }

    pub async fn start_monitoring(&self) -> std::result::Result<(), String> {
        Err("Windows USB monitoring not available on this platform".to_string())
    }
}
