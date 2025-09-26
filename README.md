# 🔌 USBWatch

A cross-platform USB device monitoring tool written in Rust that provides real-time detection of USB device connection and disconnection events on Linux, Windows, and macOS.

## ✨ Features

- **Cross-Platform Support**: Linux, Windows, macOS
- **Real-Time Monitoring**: Detect USB device events as they happen
- **Colored Output**: Modern, readable CLI output
- **Multiple Output Formats**: Plain text and JSON output
- **File Logging**: Save events to a log file
- **Built-in Installation**: Install and uninstall from system PATH
- **Lightweight**: Fast, efficient monitoring with minimal resource usage

## 🚀 Quick Start

For installation, troubleshooting, and platform-specific details, see [INSTALL.md](INSTALL.md).

### Basic Usage

```bash
# Monitor USB devices (default behaviour)
└── INSTALL.md            # Detailed installation guide

# Monitor with JSON output
usbwatch --json

# Monitor and log to file
usbwatch --logfile usb-events.log

# Monitor with both JSON and file logging
usbwatch --json --logfile usb-events.json

# Install or uninstall the CLI tool
usbwatch install
usbwatch uninstall
```

## 📋 Commands

### Monitor (Default)

```bash
usbwatch [OPTIONS]
usbwatch monitor [OPTIONS]
```

Monitor USB device events in real-time.

**Options:**

- `--json` - Output events in JSON format
- `--logfile <PATH>` - Log events to the specified file

### Install

```bash
usbwatch install
```

Install the `usbwatch` binary to your system PATH. On Unix systems, this requires administrator privileges.

### Uninstall

```bash
usbwatch uninstall
```

Remove the `usbwatch` binary from your system PATH.

## 📊 Output Examples

### Plain Text Format

```
🔌 USB Device Monitor - usbwatch v0.4.1
Press Ctrl+C to stop monitoring...
Starting USB device monitoring on Linux...
[2025-07-27 10:30:15 UTC] CONNECTED - SanDisk Ultra USB 3.0 (VID: 0781, PID: 5583) Serial: 4C530001234567891234
[2025-07-27 10:30:45 UTC] DISCONNECTED - SanDisk Ultra USB 3.0 (VID: 0781, PID: 5583) Serial: 4C530001234567891234
```

### JSON Format

```json
{
  "device_name": "SanDisk Ultra USB 3.0",
  "vendor_id": "0781",
  "product_id": "5583",
  "serial_number": "4C530001234567891234",
  "timestamp": "2025-07-27T10:30:15.123456789Z",
  "event_type": "Connected"
}
```

## 🤝 Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

### Development Setup

1. Fork the repository
2. Create a feature branch: `git checkout -b feature/my-feature`
3. Make your changes and add tests
4. Ensure code passes `cargo clippy` and `cargo test`
5. Commit using conventional commit format: `git commit -m "feat: add new feature"`
6. Push to your fork and submit a pull request

## 📄 Licence

This project is licensed under the Apache 2.0 Licence - see the [LICENSE](LICENSE) file for details.

## 🔗 Related Projects

- [lsusb](https://linux.die.net/man/8/lsusb) - List USB devices (Linux)
- [USBDeview](https://www.nirsoft.net/utils/usb_devices_view.html) - USB device viewer (Windows)
- [System Information](https://support.apple.com/en-gb/guide/system-information/welcome/mac) - macOS system information

## 📊 Performance

USBWatch is designed to be lightweight and efficient:

- Minimal CPU usage during monitoring
- Low memory footprint
- Configurable polling intervals
- No blocking operations in the main thread
