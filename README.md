# System Monitor

A lightweight Windows screen-time and activity monitor with a modern web-based GUI, written in Rust.

## Features

- **Real-time monitoring**: Tracks active applications every second
- **Browser detection**: Identifies Chrome, Edge, Firefox, and Brave browsers
- **URL extraction**: Attempts to extract current tab URLs from browser windows
- **Modern Web GUI**: Beautiful, responsive dashboard accessible via web browser
- **MS Edge App Mode**: Automatically launches in Edge app window for native feel
- **Efficient storage**: Uses hashbrown::HashMap for in-memory tracking
- **SQLite persistence**: Flushes data to database every 30 seconds
- **Low resource usage**: <1% CPU on idle, <100MB memory
- **REST API**: JSON API endpoints for dashboard data

## Requirements

- Windows 10/11
- MSYS2 MinGW64 environment
- Rust toolchain
- Microsoft Edge browser (for app mode)

## Building

1. Install MSYS2 MinGW64 if not already installed
2. Open MSYS2 MinGW64 terminal
3. Navigate to project directory
4. Build the project:

```bash
cargo build --release
```

## Running

```bash
cargo run --release
```

The monitor will:
- Create a `usage.db` SQLite database
- Start a web server on `http://localhost:3030`
- Automatically launch Edge in app mode
- Track active applications and browser tabs
- Display real-time data in the web dashboard
- Flush data to database every 30 seconds

## Web Interface

The web dashboard provides:
- **Current Activity**: Shows the active application, window title, and URL
- **Statistics**: Displays uptime, total tracked apps, and active count
- **Active Applications**: Real-time list of currently active applications
- **Recent Activity**: History of recent application usage
- **Auto-refresh**: Updates every 2 seconds automatically

### Manual Access

If Edge doesn't launch automatically, you can manually open:
- `http://localhost:3030` in any web browser
- Or use Edge app mode: `msedge --app http://localhost:3030`

## API Endpoints

- `GET /api/dashboard` - Returns current dashboard data
- `GET /api/health` - Health check endpoint
- `GET /` - Serves the main dashboard HTML
- `GET /static/*` - Serves static assets (CSS, JS)

## Database Schema

The SQLite database contains a `usage_logs` table with:
- `id`: Primary key
- `identifier`: Unique app/URL identifier
- `app_name`: Application name
- `window_title`: Window title
- `url`: Extracted URL (if browser)
- `timestamp`: Unix timestamp
- `duration`: Duration in seconds

## Technical Details

- Uses Win32 API for window detection
- Implements browser-specific URL extraction heuristics
- Batches database writes in transactions for efficiency
- Thread-safe data structures with Arc<Mutex<>>
- Graceful error handling for API calls
- Warp web server with async/await support
- Modern CSS with glassmorphism design
- Responsive JavaScript with auto-refresh

## Project Structure

```
sysmonitor/
├── src/
│   └── main.rs          # Main application with web server
├── web/
│   ├── index.html        # Dashboard HTML
│   └── static/
│       ├── style.css     # Modern CSS styling
│       └── script.js      # Dashboard JavaScript
├── Cargo.toml           # Dependencies and project config
└── README.md            # This file
```

## Future Enhancements

- Enhanced URL extraction from browser session files
- Activity categorization and reporting
- Export functionality (CSV, JSON)
- Historical data visualization with charts
- Customizable dashboard themes
- Notification system for usage alerts
