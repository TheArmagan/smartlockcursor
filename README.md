# SmartLockCursor

A Windows utility that automatically locks the mouse cursor to the display containing a fullscreen window.

## Why?

Many applications and games, especially those using borderless fullscreen mode, don't properly confine the mouse cursor to their window. This can be frustrating on multi-monitor setups where the cursor escapes to other monitors during gameplay or other fullscreen activities.

**SmartLockCursor** solves this by:
- Detecting when the foreground window is in fullscreen mode
- Automatically clipping the mouse cursor to the bounds of that monitor
- Releasing the cursor when you exit fullscreen or switch to a non-fullscreen window

## Features

- 🖥️ Multi-monitor support
- 🎮 Detects both exclusive fullscreen and borderless fullscreen windows
- ⚡ Low CPU usage (~100ms polling interval)
- 🔓 Automatically releases cursor when fullscreen exits
- 🔄 **Alt+Tab friendly** - cursor is temporarily released during Alt+Tab
- 🖱️ **Smart re-lock** - after Alt+Tab, cursor stays free until you click back on the fullscreen window
- 🛑 Clean shutdown with Ctrl+C

## Installation

### From Source

Make sure you have [Rust](https://rustup.rs/) installed, then:

```bash
git clone https://github.com/yourusername/smartlockcursor.git
cd smartlockcursor
cargo build --release
```

The executable will be at `target/release/smartlockcursor.exe`

## Usage

Simply run the executable:

```bash
smartlockcursor.exe
```

The program will:
1. Display detected monitors
2. Start monitoring for fullscreen windows
3. Automatically lock/unlock the cursor as needed

Press `Ctrl+C` to exit.

### Example Output

```
╔═══════════════════════════════════════════════════════════╗
║              SmartLockCursor v0.1.0                       ║
║  Automatically locks cursor to fullscreen windows         ║
╠═══════════════════════════════════════════════════════════╣
║  Press Ctrl+C to exit                                     ║
╚═══════════════════════════════════════════════════════════╝

[INFO] Detected 2 monitor(s):
  Monitor 1: 1920x1080 at (0, 0)
  Monitor 2: 1920x1080 at (1920, 0)

[INFO] Monitoring for fullscreen windows...

[INFO] Cursor locked to monitor: (0, 0) - (1920, 1080)
[INFO] Fullscreen exited, cursor released
```

## Running at Startup

To run SmartLockCursor automatically at Windows startup:

1. Press `Win + R`, type `shell:startup`, and press Enter
2. Create a shortcut to `smartlockcursor.exe` in this folder

Or use Task Scheduler for more control over when and how the app starts.

## How It Works

1. Every 100ms, the program checks the foreground window
2. It determines if the window covers an entire monitor (fullscreen detection)
3. If fullscreen, it uses the Windows `ClipCursor` API to confine the mouse
4. When the window exits fullscreen or loses focus, the cursor is released

### Alt+Tab Behavior

- When you press **Alt+Tab**, the cursor is immediately released for free navigation
- If you switch to a **different window**, the cursor stays free
- The cursor only re-locks when you **click back** on the fullscreen window
- This allows you to freely use other monitors after Alt+Tab without the cursor snapping back

## Building

### Requirements

- Windows 10/11
- Rust 1.70+ (uses edition 2021)

### Dependencies

- `windows` crate - Windows API bindings
- `ctrlc` crate - Ctrl+C signal handling

### Build Commands

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run directly
cargo run --release
```

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.