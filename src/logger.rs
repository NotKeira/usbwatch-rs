//! Event logging and output formatting for USB device monitoring.
//!
//! Provides modern, coloured, and structured output for USB device events in both plain text and JSON formats.
//! Supports logging to files and the console, with automatic colour detection for terminals.
//!
//! ## Features
//!
//! - Coloured output using the `colored` crate
//! - JSON and plain text output
//! - File logging
//! - Configurable via CLI options
//! - Robust error handling

use crate::device_info::UsbDeviceInfo;
use colored::*;
use std::fs::OpenOptions;
use std::io::Write;
use tokio::sync::mpsc;
/// Configuration and state for logging USB device events.
///
/// The logger handles output formatting and can write to both console
/// and log files simultaneously.
pub struct Logger {
    output_json: bool,
    log_file: Option<std::fs::File>,
    colorful: bool,
}

impl Logger {
    /// Creates a new logger instance.
    ///
    /// # Arguments
    ///
    /// * `output_json` - Whether to format output as JSON
    /// * `log_file_path` - Optional path to a log file
    /// * `colorful` - Whether to use coloured output (ignored for JSON mode)
    ///
    /// # Errors
    ///
    /// Returns an error if the log file cannot be created or opened.
    ///
    /// # Examples
    ///
    /// ```
    /// use usbwatch_rs::logger::Logger;
    ///
    /// // Console-only logger with plain text and colours
    /// let logger = Logger::new(false, None, true)?;
    ///
    /// // JSON logger with file output
    /// let logger = Logger::new(true, Some("usb-events.json"), true)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(
        output_json: bool,
        log_file_path: Option<&str>,
        colorful: bool,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let log_file = if let Some(path) = log_file_path {
            Some(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| format!("Failed to open log file '{path}': {e}"))?,
            )
        } else {
            None
        };

        Ok(Self {
            output_json,
            log_file,
            colorful,
        })
    }

    /// Logs a USB device event to console and file (if configured).
    ///
    /// The output format depends on the `output_json` setting configured
    /// during logger creation.
    ///
    /// # Arguments
    ///
    /// * `device_info` - Information about the USB device event
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialisation fails or file writing fails.
    pub fn log_device_event(
        &mut self,
        device_info: &UsbDeviceInfo,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.output_json {
            let json = serde_json::to_string(device_info)?;
            println!("{json}");
            if let Some(file) = &mut self.log_file {
                writeln!(file, "{json}")?;
                file.flush()?;
            }
        } else {
            let event_icon = match device_info.event_type {
                crate::device_info::DeviceEventType::Connected => "🔌",
                crate::device_info::DeviceEventType::Disconnected => "❌",
            };
            let styled_name = if self.colorful {
                match device_info.event_type {
                    crate::device_info::DeviceEventType::Connected => {
                        device_info.device_name.green().bold()
                    }
                    crate::device_info::DeviceEventType::Disconnected => {
                        device_info.device_name.red().bold()
                    }
                }
            } else {
                device_info.device_name.normal()
            };
            let output = format!(
                "{} {} | VID: {} PID: {} | Serial: {} | Event: {:?} | {}",
                event_icon,
                styled_name,
                device_info.vendor_id,
                device_info.product_id,
                device_info.serial_number.as_deref().unwrap_or("-"),
                device_info.event_type,
                device_info.timestamp
            );
            println!("{output}");
            if let Some(file) = &mut self.log_file {
                writeln!(file, "{output}")?;
                file.flush()?;
            }
        }
        Ok(())
    }
}

/// Async task that processes USB device events from a channel.
///
/// This function runs indefinitely, receiving device events and logging
/// them using the provided logger instance.
///
/// # Arguments
///
/// * `rx` - Receiver channel for USB device events
/// * `logger` - Logger instance for formatting and outputting events
pub async fn logger_task(mut rx: mpsc::Receiver<UsbDeviceInfo>, mut logger: Logger) {
    while let Some(device_info) = rx.recv().await {
        if let Err(e) = logger.log_device_event(&device_info) {
            eprintln!("Error logging device event: {e}");
        }
    }
}
