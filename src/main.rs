use std::path::Path;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use hashbrown::HashMap as FastHashMap;
use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use warp::Filter;
use windows::{
    Win32::Foundation::BOOL,
    Win32::System::ProcessStatus::GetProcessImageFileNameW,
    Win32::System::Threading::{OpenProcess, PROCESS_QUERY_INFORMATION},
    Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId},
};

// Configuration constants
const ACTIVITY_RETENTION_HOURS: u64 = 24; // Keep activity data for 24 hours
const MAX_RECENT_ACTIVITIES: usize = 50; // Maximum number of recent activities to show

#[derive(Debug, Clone, Serialize, Deserialize)]
struct UsageEntry {
    identifier: String,
    app_name: String,
    window_title: String,
    url: Option<String>,
    last_seen: u64,
    total_time: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ActiveEntry {
    status: bool,
    last_seen: u64,
    start_time: u64, // When this app first became active
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ApiResponse {
    success: bool,
    data: Option<serde_json::Value>,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RecentActivity {
    identifier: String,
    app_name: String,
    window_title: String,
    url: Option<String>,
    duration: u64,
    timestamp: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct DashboardData {
    current_app: Option<String>,
    current_window: Option<String>,
    current_url: Option<String>,
    active_apps: Vec<(String, u64)>,
    recent_activity: Vec<RecentActivity>,
    total_apps: usize,
    uptime: u64,
}

struct SystemMonitor {
    usage_data: Arc<Mutex<FastHashMap<String, ActiveEntry>>>,
    db_path: String,
    start_time: u64,
}

impl SystemMonitor {
    fn new() -> Self {
        Self {
            usage_data: Arc::new(Mutex::new(FastHashMap::new())),
            db_path: "usage.db".to_string(),
            start_time: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    fn init_database(&self) -> SqlResult<()> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS usage_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                identifier TEXT NOT NULL,
                app_name TEXT NOT NULL,
                window_title TEXT NOT NULL,
                url TEXT,
                timestamp INTEGER NOT NULL,
                duration INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        Ok(())
    }

    fn load_existing_data(&self) -> SqlResult<()> {
        let conn = Connection::open(&self.db_path)?;
        let mut stmt = conn.prepare("SELECT identifier, timestamp FROM usage_logs ORDER BY timestamp DESC")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })?;

        let mut usage_data = self.usage_data.lock().unwrap();
        for row in rows {
            let (identifier, timestamp) = row?;
            usage_data.insert(identifier, ActiveEntry {
                status: false,
                last_seen: timestamp as u64,
                start_time: timestamp as u64,
            });
        }
        Ok(())
    }

    fn get_foreground_window_info(&self) -> Option<(String, String, Option<String>)> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.0 == 0 {
                return None;
            }

            // Get window title
            let mut title_buffer = [0u16; 256];
            let title_len = GetWindowTextW(hwnd, &mut title_buffer);
            let window_title = if title_len > 0 {
                String::from_utf16_lossy(&title_buffer[..title_len as usize])
            } else {
                "Unknown".to_string()
            };

            // Get process ID
            let mut process_id = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut process_id));
            if process_id == 0 {
                return None;
            }

            // Get process handle
            let process_handle = OpenProcess(PROCESS_QUERY_INFORMATION, BOOL(0), process_id).ok()?;
            
            // Get process image name
            let mut image_buffer = [0u16; 260];
            let image_len = GetProcessImageFileNameW(process_handle, &mut image_buffer);
            let process_path = if image_len > 0 {
                String::from_utf16_lossy(&image_buffer[..image_len as usize])
            } else {
                return None;
            };

            // Extract executable name
            let app_name = Path::new(&process_path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("Unknown")
                .to_string();

            // Detect browser and extract URL
            let url = self.extract_browser_url(&app_name, &window_title);

            Some((app_name, window_title, url))
        }
    }

    fn extract_browser_url(&self, app_name: &str, window_title: &str) -> Option<String> {
        let app_lower = app_name.to_lowercase();
        
        if app_lower.contains("chrome") || app_lower.contains("msedge") || app_lower.contains("brave") {
            self.extract_chromium_url(app_name, window_title)
        } else if app_lower.contains("firefox") {
            self.extract_firefox_url(window_title)
        } else {
            None
        }
    }

    fn extract_chromium_url(&self, _app_name: &str, window_title: &str) -> Option<String> {
        // Try to extract URL from window title (common pattern: "Page Title - Browser Name")
        let title_parts: Vec<&str> = window_title.split(" - ").collect();
        if title_parts.len() >= 2 {
            let potential_url = title_parts.last().unwrap();
            if potential_url.starts_with("http") || potential_url.contains("://") {
                return Some(potential_url.to_string());
            }
        }

        // Try to read from Chrome's CurrentSession file
        let _user_profile = std::env::var("USERPROFILE").ok()?;
        let _session_path = if _app_name.to_lowercase().contains("msedge") {
            format!("{}\\AppData\\Local\\Microsoft\\Edge\\User Data\\Default\\Current Session", _user_profile)
        } else if _app_name.to_lowercase().contains("brave") {
            format!("{}\\AppData\\Local\\BraveSoftware\\Brave-Browser\\User Data\\Default\\Current Session", _user_profile)
        } else {
            format!("{}\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\Current Session", _user_profile)
        };

        // This is a simplified approach - in practice, you'd need to parse the binary session file
        // For now, we'll use window title heuristics
        None
    }

    fn extract_firefox_url(&self, window_title: &str) -> Option<String> {
        // Firefox often includes the URL in the window title
        // Pattern: "Page Title - Mozilla Firefox" or "Page Title | Mozilla Firefox"
        let patterns = [" - Mozilla Firefox", " | Mozilla Firefox", " — Mozilla Firefox"];
        
        for pattern in &patterns {
            if let Some(pos) = window_title.find(pattern) {
                let title_part = &window_title[..pos];
                if title_part.starts_with("http") || title_part.contains("://") {
                    return Some(title_part.to_string());
                }
            }
        }
        
        None
    }

    fn update_usage(&self, identifier: String, _app_name: String, _window_title: String, _url: Option<String>) {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut usage_data = self.usage_data.lock().unwrap();
        
        // Update existing entry or create new one
        if let Some(entry) = usage_data.get_mut(&identifier) {
            if !entry.status {
                // App just became active, set start time
                entry.start_time = current_time;
            }
            entry.status = true;
            entry.last_seen = current_time;
        } else {
            // New app, set both start time and last seen to current time
            usage_data.insert(identifier.clone(), ActiveEntry {
                status: true,
                last_seen: current_time,
                start_time: current_time,
            });
        }

        // Mark all other entries as inactive
        for (key, entry) in usage_data.iter_mut() {
            if *key != identifier {
                entry.status = false;
            }
        }
    }

    fn flush_to_database(&self) -> SqlResult<()> {
        let mut conn = Connection::open(&self.db_path)?;
        let usage_data = self.usage_data.lock().unwrap();
        
        let tx = conn.transaction()?;
        
        for (identifier, entry) in usage_data.iter() {
            if entry.status {
                // Calculate total duration since app became active
                let current_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();
                let duration = current_time.saturating_sub(entry.start_time);
                
                if duration > 0 {
                    // Extract app name and window title from identifier
                    let (app_name, window_title, url) = if let Some((app, rest)) = identifier.split_once(':') {
                        if rest.starts_with("http") {
                            (app.to_string(), rest.to_string(), Some(rest.to_string()))
                        } else {
                            (app.to_string(), rest.to_string(), None)
                        }
                    } else {
                        (identifier.clone(), "Unknown".to_string(), None)
                    };

                    tx.execute(
                        "INSERT INTO usage_logs (identifier, app_name, window_title, url, timestamp, duration) 
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                        params![
                            identifier,
                            app_name,
                            window_title,
                            url.unwrap_or_default(),
                            current_time,
                            duration
                        ],
                    )?;
                }
            }
        }
        
        tx.commit()?;
        Ok(())
    }

    fn get_recent_activity(&self) -> Vec<RecentActivity> {
        // Get recent activity from the last 24 hours (configurable retention period)
        let conn = match Connection::open(&self.db_path) {
            Ok(conn) => conn,
            Err(_) => return Vec::new(),
        };

        let mut stmt = match conn.prepare(
            &format!("SELECT identifier, app_name, window_title, url, duration, timestamp 
             FROM usage_logs 
             WHERE timestamp >= ?1 
             ORDER BY timestamp DESC 
             LIMIT {}", MAX_RECENT_ACTIVITIES)
        ) {
            Ok(stmt) => stmt,
            Err(_) => return Vec::new(),
        };

        // Get retention period ago timestamp (persistent for configured hours)
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let retention_cutoff = current_time - (ACTIVITY_RETENTION_HOURS * 3600); // Convert hours to seconds

        let rows = match stmt.query_map([retention_cutoff as i64], |row| {
            Ok(RecentActivity {
                identifier: row.get::<_, String>(0)?,
                app_name: row.get::<_, String>(1)?,
                window_title: row.get::<_, String>(2)?,
                url: row.get::<_, Option<String>>(3)?,
                duration: row.get::<_, i64>(4)? as u64,
                timestamp: row.get::<_, i64>(5)? as u64,
            })
        }) {
            Ok(rows) => rows,
            Err(_) => return Vec::new(),
        };

        let mut activities = Vec::new();
        for row in rows {
            if let Ok(activity) = row {
                activities.push(activity);
            }
        }
        activities
    }

    fn get_dashboard_data(&self) -> DashboardData {
        let usage_data = self.usage_data.lock().unwrap();
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let mut current_app = None;
        let mut current_window = None;
        let mut current_url = None;
        let mut active_apps = Vec::new();

        for (identifier, entry) in usage_data.iter() {
            if entry.status {
                // Calculate total duration since app became active
                let duration = current_time.saturating_sub(entry.start_time);
                active_apps.push((identifier.clone(), duration));
                
                // Extract app info from identifier
                if let Some((app, rest)) = identifier.split_once(':') {
                    current_app = Some(app.to_string());
                    if rest.starts_with("http") {
                        current_url = Some(rest.to_string());
                    } else {
                        current_window = Some(rest.to_string());
                    }
                }
            }
        }

        // Sort by duration (most recent first)
        active_apps.sort_by(|a, b| b.1.cmp(&a.1));

        // Get recent activity from database
        let recent_activity = self.get_recent_activity();

        DashboardData {
            current_app,
            current_window,
            current_url,
            active_apps,
            recent_activity,
            total_apps: usage_data.len(),
            uptime: current_time - self.start_time,
        }
    }

    fn print_status(&self) {
        let dashboard_data = self.get_dashboard_data();
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        println!("\n=== System Monitor Status ===");
        println!("Timestamp: {}", current_time);
        println!("Uptime: {} seconds", dashboard_data.uptime);
        
        if let Some(ref app) = dashboard_data.current_app {
            println!("Current App: {}", app);
        }
        if let Some(ref window) = dashboard_data.current_window {
            println!("Window: {}", window);
        }
        if let Some(ref url) = dashboard_data.current_url {
            println!("URL: {}", url);
        }
        
        println!("Active Applications:");
        for (identifier, duration) in &dashboard_data.active_apps {
            println!("  ✓ {} (active for {}s)", identifier, duration);
        }
        
        println!("Total tracked applications: {}", dashboard_data.total_apps);
    }

    async fn run_monitoring(&self) {
        let mut last_flush = SystemTime::now();
        let flush_interval = Duration::from_secs(5); // Flush every 5 seconds for faster updates
        
        loop {
            if let Some((app_name, window_title, url)) = self.get_foreground_window_info() {
                let identifier = if let Some(ref url) = url {
                    format!("{}:{}", app_name, url)
                } else {
                    format!("{}:{}", app_name, window_title)
                };
                
                self.update_usage(identifier, app_name, window_title, url);
            }
            
            // Print status every 5 seconds for faster debugging
            let now = SystemTime::now();
            if now.duration_since(last_flush).unwrap() >= Duration::from_secs(5) {
                self.print_status();
            }
            
            // Flush to database every 5 seconds for faster updates
            if now.duration_since(last_flush).unwrap() >= flush_interval {
                if let Err(e) = self.flush_to_database() {
                    eprintln!("Error flushing to database: {}", e);
                } else {
                    println!("Data flushed to database");
                }
                last_flush = now;
            }
            
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
}

fn launch_edge_app() -> Result<(), Box<dyn std::error::Error>> {
    let url = "http://localhost:3030";
    let edge_path = r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe";
    
    Command::new(edge_path)
        .args(&[
            "--app",
            &url,
            "--new-window",
            "--disable-web-security",
            "--disable-features=VizDisplayCompositor"
        ])
        .spawn()?;
    
    println!("Launched Edge app window at {}", url);
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("System Monitor v0.1.0 with Web GUI");
    println!("Starting web server and monitoring...");
    
    let monitor = Arc::new(SystemMonitor::new());
    
    // Initialize database
    monitor.init_database()?;
    monitor.load_existing_data()?;
    
    println!("Database initialized. Starting web server on http://localhost:3030");
    
    // Clone monitor for web server
    let monitor_clone = monitor.clone();
    
    // Start monitoring in background
    let monitor_task = tokio::spawn(async move {
        monitor_clone.run_monitoring().await;
    });
    
    // Start web server
    let web_server_task = tokio::spawn(async move {
        start_web_server(monitor).await;
    });
    
    // Launch Edge app window
    tokio::task::spawn_blocking(|| {
        std::thread::sleep(Duration::from_secs(2)); // Wait for server to start
        if let Err(e) = launch_edge_app() {
            eprintln!("Failed to launch Edge app: {}", e);
            println!("You can manually open http://localhost:3030 in your browser");
        }
    });
    
    // Wait for both tasks
    tokio::try_join!(monitor_task, web_server_task)?;
    
    Ok(())
}

async fn start_web_server(monitor: Arc<SystemMonitor>) {
    let monitor_filter = warp::any().map(move || monitor.clone());
    
    // Serve static files
    let static_files = warp::path("static")
        .and(warp::fs::dir("web/static"));
    
    // API routes
    let api_routes = warp::path("api")
        .and(
            // Dashboard data endpoint
            warp::path("dashboard")
                .and(warp::get())
                .and(monitor_filter.clone())
                .and_then(handle_dashboard)
                .or(
                    // Health check endpoint
                    warp::path("health")
                        .and(warp::get())
                        .and_then(handle_health)
                )
        );
    
    // Serve main HTML page
    let index = warp::path::end()
        .and(warp::get())
        .and(warp::fs::file("web/index.html"));
    
    let routes = index
        .or(static_files)
        .or(api_routes);
    
    println!("Web server starting on http://localhost:3030");
    warp::serve(routes)
        .run(([127, 0, 0, 1], 3030))
        .await;
}

async fn handle_dashboard(monitor: Arc<SystemMonitor>) -> Result<impl warp::Reply, warp::Rejection> {
    let data = monitor.get_dashboard_data();
    Ok(warp::reply::json(&ApiResponse {
        success: true,
        data: Some(serde_json::to_value(data).unwrap()),
        error: None,
    }))
}

async fn handle_health() -> Result<impl warp::Reply, warp::Rejection> {
    Ok(warp::reply::json(&ApiResponse {
        success: true,
        data: Some(serde_json::json!({"status": "healthy"})),
        error: None,
    }))
}