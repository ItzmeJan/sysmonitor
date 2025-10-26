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
        this.updateRecentActivity(data.active_apps);

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

    updateRecentActivity(activeApps) {
        const container = document.getElementById('recent-activity-list');
        
        if (activeApps.length === 0) {
            container.innerHTML = '<div class="no-data">No recent activity</div>';
            return;
        }

        // Show last 5 activities
        const recentApps = activeApps.slice(0, 5);
        
        container.innerHTML = recentApps.map(app => {
            const [identifier, duration] = app;
            const [appName, ...rest] = identifier.split(':');
            const details = rest.join(':');
            
            return `
                <div class="activity-entry">
                    <div class="activity-info">
                        <div class="activity-app">${this.escapeHtml(appName)}</div>
                        <div class="activity-details">${this.escapeHtml(details)}</div>
                    </div>
                    <div class="activity-time">${this.formatDuration(duration)}</div>
                </div>
            `;
        }).join('');
    }

    formatDuration(seconds) {
        if (seconds < 60) {
            return `${seconds}s`;
        } else if (seconds < 3600) {
            const minutes = Math.floor(seconds / 60);
            const remainingSeconds = seconds % 60;
            return `${minutes}m ${remainingSeconds}s`;
        } else {
            const hours = Math.floor(seconds / 3600);
            const minutes = Math.floor((seconds % 3600) / 60);
            return `${hours}h ${minutes}m`;
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
