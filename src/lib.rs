//! # USBWatch
//!
//! A cross-platform USB device monitoring library and command-line tool.
//!
//! USBWatch provides real-time monitoring of USB device connection and disconnection events on Linux, Windows, and macOS. It offers both a library API for integration into other applications and a standalone command-line tool.
//!
//! ## Features
//!
//! - **Cross-platform**: Linux (sysfs), Windows (Win32 APIs), macOS (IOKit)
//! - **Real-time monitoring**: Detect USB events as they happen
//! - **Multiple output formats**: Plain text and JSON
//! - **File logging**: Save events to log files
//! - **Coloured output**: Modern, readable CLI output
//! - **Async/await support**: Built with Tokio for efficient I/O
//! - **Device handle traits**: Access platform-specific device handles for advanced operations
//! - **Install/uninstall commands**: Manage CLI tool from the command line
//!
//! ## Quick Start
//!
//! ### Command Line Usage
//!
//! ```bash
//! # Monitor USB devices with default output
//! usbwatch
//!
//! # Monitor with JSON output
//! usbwatch --json
//!
//! # Monitor and log to file
//! usbwatch --logfile usb-events.log
//!
//! # Monitor with coloured output (default if supported)
//! usbwatch
//!
//! # Install or uninstall the CLI tool
//! usbwatch install
//! usbwatch uninstall
//! ```
//!
//! ### Library Usage
//!
//! ```rust,no_run
//! use usbwatch_rs::{UsbWatcher, UsbDeviceInfo, AsDeviceHandle, DeviceHandle};
//! use tokio::sync::mpsc;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let (tx, mut rx) = mpsc::channel(100);
//!     let watcher = UsbWatcher::new(tx)?;
//!
//!     // Start monitoring in a background task
//!     tokio::spawn(async move {
//!         if let Err(e) = watcher.start_monitoring().await {
//!             eprintln!("Monitoring error: {}", e);
//!         }
//!     });
//!
//!     // Process device events
//!     while let Some(device_info) = rx.recv().await {
//!         println!("Device event: {}", device_info);
//!         // Access platform-specific device handle
//!         match device_info.as_device_handle() {
//!             #[cfg(target_os = "linux")]
//!             DeviceHandle::Linux { sysfs_path, device_node } => {
//!                 println!("Linux sysfs path: {}", sysfs_path);
//!                 if let Some(node) = device_node {
//!                     println!("Device node: {}", node);
//!                 }
//!             }
//!             #[cfg(target_os = "windows")]
//!             DeviceHandle::Windows { instance_id, interface_path } => {
//!                 println!("Windows instance ID: {}", instance_id);
//!                 if let Some(path) = interface_path {
//!                     println!("Interface path: {}", path);
//!                 }
//!             }
//!             #[cfg(target_os = "macos")]
//!             DeviceHandle::Macos { device_id } => {
//!                 println!("macOS device ID: {}", device_id);
//!             }
//!             _ => {
//!                 println!("No platform-specific handle available");
//!             }
//!         }
//!     }
//!
//!     Ok(())
//! }
//! ```
//!
//! ## Library API Highlights
//!
//! - [`UsbWatcher`] - Cross-platform watcher for USB device events
//! - [`UsbDeviceInfo`] - Struct containing device metadata and event info
//! - [`DeviceHandle`] - Enum for platform-specific device handles
//! - [`AsDeviceHandle`] - Trait for accessing device handles from device info
//! - [`create_watcher`] - Convenience function for watcher creation
//! - [`monitor_with_callback`] - High-level async monitoring with callback
//! - [`monitor_for_duration`] - Collect events for a fixed duration
//!
//! ## Platform Support
//!
//! - **Linux**: Uses sysfs filesystem (`/sys/bus/usb/devices`)
//! - **Windows**: Uses Win32 Device Installation APIs
//!
//! ## Error Handling
//!
//! All public APIs use `Result` types for proper error handling. Platform-specific
//! errors are wrapped in boxed `std::error::Error` for consistency.

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod device_info;
pub mod logger;
pub mod watcher;

// Re-export commonly used types
pub use device_info::{AsDeviceHandle, DeviceEventType, DeviceHandle, UsbDeviceInfo};
pub use logger::{logger_task, Logger};
pub use watcher::UsbWatcher;

/// Library version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Library name
pub const NAME: &str = env!("CARGO_PKG_NAME");

/// Library description
pub const DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

/// A result type for USB monitoring operations
pub type Result<T> = std::result::Result<T, String>;

/// Create a new USB watcher with the given channel sender.
///
/// This is a convenience function that creates a new [`UsbWatcher`] instance.
///
/// # Arguments
///
/// * `sender` - The channel sender to send USB device events to
///
/// # Returns
///
/// Returns a [`Result`] containing the [`UsbWatcher`] or an error if the watcher
/// cannot be created (e.g., on unsupported platforms).
///
/// # Examples
///
/// ```rust,no_run
/// use usbwatch_rs::{create_watcher, UsbDeviceInfo};
/// use tokio::sync::mpsc;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let (tx, mut rx) = mpsc::channel::<UsbDeviceInfo>(100);
///     let watcher = create_watcher(tx)?;
///     
///     // Use the watcher...
///     Ok(())
/// }
/// ```
pub fn create_watcher(sender: tokio::sync::mpsc::Sender<UsbDeviceInfo>) -> Result<UsbWatcher> {
    UsbWatcher::new(sender).map_err(|e| e.to_string())
}

/// Start monitoring USB devices with a callback function.
///
/// This is a high-level convenience function that sets up monitoring and calls
/// the provided callback for each USB device event.
///
/// # Arguments
///
/// * `callback` - A function that will be called for each USB device event
///
/// # Returns
///
/// Returns a [`Result`] that completes when monitoring stops or encounters an error.
///
/// # Examples
///
/// ```rust,no_run
/// use usbwatch_rs::{monitor_with_callback, UsbDeviceInfo};
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     monitor_with_callback(|device_info| {
///         println!("USB event: {}", device_info);
///     }).await?;
///     
///     Ok(())
/// }
/// ```
pub async fn monitor_with_callback<F>(mut callback: F) -> Result<()>
where
    F: FnMut(UsbDeviceInfo) + Send + 'static,
{
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let watcher = create_watcher(tx)?;

    // Process events with callback in background
    let callback_handle = tokio::spawn(async move {
        while let Some(device_info) = rx.recv().await {
            callback(device_info);
        }
    });

    // Start monitoring (this will block until monitoring completes)
    let monitoring_result = watcher.start_monitoring().await;

    // Stop callback processing
    callback_handle.abort();

    monitoring_result.map_err(|e| e.to_string())
}

/// Start monitoring USB devices and collect events into a vector.
///
/// This function monitors for the specified duration and returns all collected events.
/// Useful for testing or collecting a snapshot of USB activity.
///
/// # Arguments
///
/// * `duration` - How long to monitor for events
///
/// # Returns
///
/// Returns a [`Result`] containing a vector of [`UsbDeviceInfo`] events collected
/// during the monitoring period.
///
/// # Examples
///
/// ```rust,no_run
/// use usbwatch_rs::monitor_for_duration;
/// use std::time::Duration;
///
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let events = monitor_for_duration(Duration::from_secs(5)).await?;
///     println!("Collected {} USB events", events.len());
///     
///     for event in events {
///         println!("Event: {}", event);
///     }
///     
///     Ok(())
/// }
/// ```
pub async fn monitor_for_duration(duration: std::time::Duration) -> Result<Vec<UsbDeviceInfo>> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);
    let watcher = create_watcher(tx)?;
    let mut events = Vec::new();

    // Collect events in background
    let collection_handle = tokio::spawn(async move {
        let mut collected = Vec::new();
        let timeout = tokio::time::sleep(duration);
        tokio::pin!(timeout);

        loop {
            tokio::select! {
                device_info = rx.recv() => {
                    if let Some(device_info) = device_info {
                        collected.push(device_info);
                    } else {
                        break; // Channel closed
                    }
                }
                _ = &mut timeout => {
                    break; // Duration elapsed
                }
            }
        }
        collected
    });

    // Start monitoring task (but we'll collect separately)
    let monitoring_task = async move { watcher.start_monitoring().await };

    // Wait for collection to complete
    tokio::select! {
        collected = collection_handle => {
            events = collected.map_err(|e| e.to_string())?;
        }
        result = monitoring_task => {
            if let Err(e) = result {
                return Err(e.to_string());
            }
        }
    }

    Ok(events)
}

/// Check if USB monitoring is supported on the current platform.
///
/// # Returns
///
/// Returns `true` if USB monitoring is supported on this platform, `false` otherwise.
///
/// # Examples
///
/// ```rust
/// use usbwatch_rs::is_supported;
///
/// if is_supported() {
///     println!("USB monitoring is supported on this platform");
/// } else {
///     println!("USB monitoring is not supported on this platform");
/// }
/// ```
pub fn is_supported() -> bool {
    cfg!(target_os = "linux") || cfg!(target_os = "windows")
}

/// Get information about the current platform's USB monitoring implementation.
///
/// # Returns
///
/// Returns a string describing the platform-specific implementation used
/// for USB monitoring.
///
/// # Examples
///
/// ```rust
/// use usbwatch_rs::platform_info;
///
/// println!("USB monitoring implementation: {}", platform_info());
/// ```
pub fn platform_info() -> &'static str {
    if cfg!(target_os = "linux") {
        "Linux sysfs (/sys/bus/usb/devices)"
    } else if cfg!(target_os = "windows") {
        "Windows Win32 Device Installation APIs"
    } else {
        "Unsupported platform"
    }
}
