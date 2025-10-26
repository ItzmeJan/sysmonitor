class SystemMonitorDashboard {
    constructor() {
        this.updateInterval = 2000; // Update every 2 seconds
        this.lastUpdateTime = null;
        this.init();
    }

    init() {
        this.loadDashboardData();
        this.startAutoUpdate();
        this.updateLastUpdatedTime();
    }

    async loadDashboardData() {
        try {
            const response = await fetch('/api/dashboard');
            const result = await response.json();
            
            if (result.success && result.data) {
                this.updateDashboard(result.data);
            } else {
                console.error('Failed to load dashboard data:', result.error);
                this.showError('Failed to load dashboard data');
            }
        } catch (error) {
            console.error('Error fetching dashboard data:', error);
            this.showError('Connection error');
        }
    }

    updateDashboard(data) {
        // Update current activity
        this.updateElement('current-app', data.current_app || '-');
        this.updateElement('current-window', data.current_window || '-');
        
        const urlItem = document.getElementById('url-item');
        const currentUrl = document.getElementById('current-url');
        
        if (data.current_url) {
            urlItem.style.display = 'block';
            currentUrl.textContent = data.current_url;
            currentUrl.style.color = 'var(--accent-color)';
            currentUrl.style.cursor = 'pointer';
            currentUrl.onclick = () => window.open(data.current_url, '_blank');
        } else {
            urlItem.style.display = 'none';
        }

        // Update statistics
        this.updateElement('uptime', this.formatDuration(data.uptime));
        this.updateElement('total-apps', data.total_apps);
        this.updateElement('active-count', data.active_apps.length);

        // Update active applications list
        this.updateActiveAppsList(data.active_apps);

        // Update recent activity (using active apps as recent activity for now)
        this.updateRecentActivity(data.recent_activity);

        this.updateLastUpdatedTime();
    }

    updateElement(id, value) {
        const element = document.getElementById(id);
        if (element) {
            element.textContent = value;
        }
    }

    updateActiveAppsList(activeApps) {
        const container = document.getElementById('active-apps-list');
        
        if (activeApps.length === 0) {
            container.innerHTML = '<div class="no-data">No active applications</div>';
            return;
        }

        container.innerHTML = activeApps.map(app => {
            const [identifier, duration] = app;
            const [appName, ...rest] = identifier.split(':');
            const details = rest.join(':');
            
            return `
                <div class="app-item">
                    <div class="app-name" title="${identifier}">
                        <strong>${this.escapeHtml(appName)}</strong>
                        ${details ? `<br><small>${this.escapeHtml(details)}</small>` : ''}
                    </div>
                    <div class="app-duration">${this.formatDuration(duration)}</div>
                </div>
            `;
        }).join('');
    }

    updateRecentActivity(recentActivity) {
        const container = document.getElementById('recent-activity-list');
        
        if (recentActivity.length === 0) {
            container.innerHTML = '<div class="no-data">No recent activity</div>';
            return;
        }

        // Group activities by browser first, then by site within each browser
        const groupedActivities = this.groupActivitiesByBrowserAndSite(recentActivity);

        container.innerHTML = groupedActivities.map(browserGroup => {
            const browserTotalTime = browserGroup.total_duration;
            const latestActivity = browserGroup.latest_activity;
            const timeAgo = this.formatTimeAgo(latestActivity.timestamp);
            
            // Create individual site entries
            const siteEntries = browserGroup.sites.map(site => {
                return `
                    <div class="site-entry">
                        <div class="site-name">${this.escapeHtml(site.site_name)}</div>
                        <div class="site-duration">${this.formatDuration(site.duration)}</div>
                    </div>
                `;
            }).join('');
            
            return `
                <div class="browser-group">
                    <div class="browser-header">
                        <div class="browser-info">
                            <div class="browser-name">${this.escapeHtml(browserGroup.app_name)}</div>
                            <div class="browser-sites">${browserGroup.sites.length} sites</div>
                        </div>
                        <div class="browser-time">
                            <div class="browser-duration">${this.formatDuration(browserTotalTime)}</div>
                            <div class="browser-time-ago">${timeAgo}</div>
                        </div>
                    </div>
                    <div class="sites-list">
                        ${siteEntries}
                    </div>
                </div>
            `;
        }).join('');
    }

    groupActivitiesByBrowserAndSite(activities) {
        const browserGroups = new Map();
        
        // First, group by browser
        activities.forEach(activity => {
            const appName = activity.app_name;
            
            if (!browserGroups.has(appName)) {
                browserGroups.set(appName, {
                    app_name: appName,
                    total_duration: 0,
                    latest_activity: activity,
                    sites: new Map()
                });
            }
            
            const browserGroup = browserGroups.get(appName);
            browserGroup.total_duration += activity.duration;
            
            // Keep track of the most recent activity for this browser
            if (activity.timestamp > browserGroup.latest_activity.timestamp) {
                browserGroup.latest_activity = activity;
            }
            
            // Group sites within this browser
            const siteName = activity.url || activity.window_title;
            if (!browserGroup.sites.has(siteName)) {
                browserGroup.sites.set(siteName, {
                    site_name: siteName,
                    duration: 0
                });
            }
            browserGroup.sites.get(siteName).duration += activity.duration;
        });
        
        // Convert to array format and sort
        return Array.from(browserGroups.values()).map(browserGroup => {
            // Convert sites Map to array and sort by duration (highest first)
            const sitesArray = Array.from(browserGroup.sites.values())
                .sort((a, b) => b.duration - a.duration);
            
            return {
                app_name: browserGroup.app_name,
                total_duration: browserGroup.total_duration,
                latest_activity: browserGroup.latest_activity,
                sites: sitesArray
            };
        }).sort((a, b) => b.latest_activity.timestamp - a.latest_activity.timestamp);
    }

    groupActivitiesByApp(activities) {
        const groups = new Map();
        
        activities.forEach(activity => {
            const appName = activity.app_name;
            if (!groups.has(appName)) {
                groups.set(appName, {
                    app_name: appName,
                    activities: []
                });
            }
            groups.get(appName).activities.push(activity);
        });

        // Sort activities within each group by timestamp (most recent first)
        groups.forEach(group => {
            group.activities.sort((a, b) => b.timestamp - a.timestamp);
        });

        // Convert to array and sort groups by most recent activity
        return Array.from(groups.values()).sort((a, b) => 
            b.activities[0].timestamp - a.activities[0].timestamp
        );
    }

    formatDuration(seconds) {
        if (seconds < 60) {
            return `${seconds}s`;
        } else if (seconds < 3600) {
            const minutes = Math.floor(seconds / 60);
            const remainingSeconds = seconds % 60;
            if (remainingSeconds === 0) {
                return `${minutes}m`;
            }
            return `${minutes}m ${remainingSeconds}s`;
        } else {
            const hours = Math.floor(seconds / 3600);
            const minutes = Math.floor((seconds % 3600) / 60);
            if (minutes === 0) {
                return `${hours}h`;
            }
            return `${hours}h ${minutes}m`;
        }
    }

    formatTimeAgo(timestamp) {
        const now = Math.floor(Date.now() / 1000);
        const diff = now - timestamp;
        
        if (diff < 60) {
            return 'Just now';
        } else if (diff < 3600) {
            const minutes = Math.floor(diff / 60);
            return `${minutes}m ago`;
        } else if (diff < 86400) {
            const hours = Math.floor(diff / 3600);
            return `${hours}h ago`;
        } else {
            const days = Math.floor(diff / 86400);
            return `${days}d ago`;
        }
    }

    updateLastUpdatedTime() {
        const now = new Date();
        const timeString = now.toLocaleTimeString();
        this.updateElement('last-updated', timeString);
    }

    startAutoUpdate() {
        setInterval(() => {
            this.loadDashboardData();
        }, this.updateInterval);
    }

    showError(message) {
        // Update status indicator
        const statusDot = document.querySelector('.status-dot');
        const statusText = document.getElementById('status-text');
        
        statusDot.classList.remove('active');
        statusText.textContent = message;
        
        // Reset after 5 seconds
        setTimeout(() => {
            statusDot.classList.add('active');
            statusText.textContent = 'Monitoring Active';
        }, 5000);
    }

    escapeHtml(text) {
        const div = document.createElement('div');
        div.textContent = text;
        return div.innerHTML;
    }
}

// Initialize dashboard when page loads
document.addEventListener('DOMContentLoaded', () => {
    new SystemMonitorDashboard();
});

// Add some visual feedback for user interactions
document.addEventListener('click', (e) => {
    if (e.target.classList.contains('app-duration') || e.target.classList.contains('activity-time')) {
        e.target.style.transform = 'scale(0.95)';
        setTimeout(() => {
            e.target.style.transform = 'scale(1)';
        }, 150);
    }
});

// Handle window focus/blur for better performance
let isPageVisible = true;
document.addEventListener('visibilitychange', () => {
    isPageVisible = !document.hidden;
});

// Add keyboard shortcuts
document.addEventListener('keydown', (e) => {
    if (e.ctrlKey && e.key === 'r') {
        e.preventDefault();
        location.reload();
    }
});
