//! macOS-specific USB device watcher implementation.
//!
//! Uses IOKit FFI to detect USB device events in real time. Supports coloured output and modern CLI integration.

#[cfg(target_os = "macos")]
use crate::device_info::{DeviceEventType, DeviceHandle, UsbDeviceInfo};
#[cfg(target_os = "macos")]
use core_foundation::base::CFRelease;
#[cfg(target_os = "macos")]
use core_foundation::number::{kCFNumberSInt16Type, CFNumberGetValue, CFNumberRef};
#[cfg(target_os = "macos")]
use core_foundation::string::{CFString, CFStringRef};
#[cfg(target_os = "macos")]
use io_kit_sys::types::*;
#[cfg(target_os = "macos")]
use io_kit_sys::*;
#[cfg(target_os = "macos")]
use std::ffi::CStr;
#[cfg(target_os = "macos")]
use tokio::sync::mpsc;

#[cfg(target_os = "macos")]
/// Watches for USB device events on macOS using IOKit.
///
/// This struct provides asynchronous monitoring of USB device connections and disconnections
/// on macOS, sending events through a Tokio channel.
pub struct MacosUsbWatcher {
    tx: mpsc::Sender<UsbDeviceInfo>,
}

#[cfg(target_os = "macos")]
impl MacosUsbWatcher {
    /// Creates a new `MacosUsbWatcher` with the given channel sender.
    ///
    /// # Arguments
    ///
    /// * `tx` - Tokio channel sender for publishing USB device events.
    pub fn new(tx: mpsc::Sender<UsbDeviceInfo>) -> Self {
        Self { tx }
    }

    /// Starts monitoring USB devices on macOS.
    ///
    /// Enumerates currently connected USB devices and sends their info through the channel.
    /// In a full implementation, this would register for device notifications and run the event loop.
    ///
    /// # Errors
    ///
    /// Returns an error if IOKit FFI calls fail or device enumeration cannot be performed.
    pub async fn start_monitoring(&self) -> Result<(), String> {
        println!("Starting USB device monitoring on macOS...");
        // SAFETY: FFI calls to IOKit
        unsafe {
            let matching_dict = IOServiceMatching(b"IOUSBDevice\0".as_ptr() as *const i8);
            if matching_dict.is_null() {
                return Err("Failed to create matching dictionary for IOUSBDevice".to_string());
            }

            let mut iter: io_iterator_t = 0;
            let kr = IOServiceGetMatchingServices(kIOMasterPortDefault, matching_dict, &mut iter);
            if kr != 0 {
                return Err(format!("IOServiceGetMatchingServices failed: {kr}"));
            }

            loop {
                let device = IOIteratorNext(iter);
                if device == 0 {
                    break;
                }

                // Get device name
                let mut device_name_buf = [0i8; 128];
                let kr = IORegistryEntryGetName(device, device_name_buf.as_mut_ptr());
                let device_name = if kr == 0 {
                    CStr::from_ptr(device_name_buf.as_ptr())
                        .to_string_lossy()
                        .into_owned()
                } else {
                    "Unknown USB Device".to_string()
                };

                // Extract vendor and product IDs from device properties
                let vendor_id = self
                    .get_device_property_u16(device, b"idVendor\0")
                    .map(|id| format!("{:04x}", id))
                    .unwrap_or_else(|| "0000".to_string());

                let product_id = self
                    .get_device_property_u16(device, b"idProduct\0")
                    .map(|id| format!("{:04x}", id))
                    .unwrap_or_else(|| "0000".to_string());

                // Try to get serial number
                let serial_number = self.get_device_property_string(device, b"USB Serial Number\0");

                let info = UsbDeviceInfo {
                    device_name,
                    vendor_id,
                    product_id,
                    serial_number,
                    timestamp: chrono::Utc::now(),
                    event_type: DeviceEventType::Connected,
                    device_handle: DeviceHandle::Macos {
                        device_id: format!("{device}"),
                    },
                };
                let _ = self.tx.send(info).await;
                IOObjectRelease(device);
            }
            IOObjectRelease(iter);
        }
        Ok(())
    }

    /// Helper function to get a 16-bit integer property from an IOKit device
    unsafe fn get_device_property_u16(
        &self,
        device: io_object_t,
        property_name: &[u8],
    ) -> Option<u16> {
        let prop_name = core_foundation::string::CFString::from_static_string(
            std::str::from_utf8(property_name)
                .ok()?
                .trim_end_matches('\0'),
        );
        let prop = IORegistryEntryCreateCFProperty(
            device,
            prop_name.as_concrete_TypeRef(),
            std::ptr::null_mut(),
            0,
        );

        if prop.is_null() {
            return None;
        }

        // Convert CFNumber to u16
        let cf_number = prop as core_foundation::number::CFNumberRef;
        let mut value: u16 = 0;
        if core_foundation::number::CFNumberGetValue(
            cf_number,
            core_foundation::number::kCFNumberSInt16Type,
            &mut value as *mut u16 as *mut std::ffi::c_void,
        ) {
            core_foundation::base::CFRelease(prop);
            Some(value)
        } else {
            core_foundation::base::CFRelease(prop);
            None
        }
    }

    /// Helper function to get a string property from an IOKit device
    unsafe fn get_device_property_string(
        &self,
        device: io_object_t,
        property_name: &[u8],
    ) -> Option<String> {
        let prop_name = core_foundation::string::CFString::from_static_string(
            std::str::from_utf8(property_name)
                .ok()?
                .trim_end_matches('\0'),
        );
        let prop = IORegistryEntryCreateCFProperty(
            device,
            prop_name.as_concrete_TypeRef(),
            std::ptr::null_mut(),
            0,
        );

        if prop.is_null() {
            return None;
        }

        // Convert CFString to Rust String
        let cf_string = prop as core_foundation::string::CFStringRef;
        let rust_string =
            core_foundation::string::CFString::wrap_under_create_rule(cf_string).to_string();

        if rust_string.is_empty() {
            None
        } else {
            Some(rust_string)
        }
    }
}
