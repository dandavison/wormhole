// Wormhole GitHub/JIRA Integration
// Adds Terminal, Cursor, VSCode buttons and cross-linking to GitHub and JIRA pages

const WORMHOLE_PORT = 7117;
const WORMHOLE_BASE = `http://localhost:${WORMHOLE_PORT}`;

// Cache describe result for current page
let cachedDescribe = null;
let cachedUrl = null;

// Prevent concurrent injection
let injecting = false;

// VSCode iframe state
let vscodeExpanded = false;
let vscodeMaximized = false;

function isGitHubPage() {
    return window.location.hostname === 'github.com';
}

function isJiraPage() {
    return window.location.hostname.endsWith('.atlassian.net');
}

async function getDescribe() {
    if (cachedUrl === window.location.href && cachedDescribe) {
        return cachedDescribe;
    }
    try {
        const resp = await fetch(`${WORMHOLE_BASE}/project/describe`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ url: window.location.href })
        });
        if (resp.ok) {
            cachedDescribe = await resp.json();
            cachedUrl = window.location.href;
            return cachedDescribe;
        }
    } catch (err) {
        console.warn('[Wormhole] describe error:', err.message);
    }
    return null;
}

function createButtons(info) {
    const container = document.createElement('div');
    container.className = 'wormhole-buttons';

    let html = '';

    // Cross-platform link first
    if (isJiraPage() && info?.github_url && info?.github_label) {
        html += `<a class="wormhole-link wormhole-link-github" href="${info.github_url}" title="Open GitHub PR">${info.github_label}</a>`;
    }
    if (isGitHubPage() && info?.jira_url && info?.jira_key) {
        html += `<a class="wormhole-link wormhole-link-jira" href="${info.jira_url}" title="Open JIRA">${info.jira_key}</a>`;
    }

    // Terminal/Cursor/VSCode buttons if we have a task/project
    if (info?.name && info?.kind) {
        html += `
            <button class="wormhole-btn wormhole-btn-icon wormhole-btn-terminal" title="Open in Terminal"><img src="https://cdn.jim-nielsen.com/macos/1024/terminal-2021-06-03.png?rf=1024" alt="Terminal"></button>
            <button class="wormhole-btn wormhole-btn-icon wormhole-btn-cursor" title="Open in Cursor"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAMAAACdt4HsAAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAGhUExURff39Pn59vj49dPTz42Mh/r69+Pj4JOTjj8+OCMiG+/v662tqFFQSiYlHiQjHPX18sTEwGdmYCsqJGdmYcXEwPb28tjY1IB/ejU0LSUkHYCAeufn45qalUNCPENDPPHx7rSzr1ZWUCcmH8rKxm1tZy4tJm5tZ9zc2YeHgTg3MYeHgurq56GhnEhHQfPz8Lq6tlxcVignIN3d2XV1by8uJyEgGICAeyEgGT08NVJRS09OSDs6M/X18WloY1ZVT76+uurq5uzs6O3t6qSjn2ppYyIhGjo5MomIg97e2vPy73JxbCAfGElIQaKinba2sikoIV1cVru6tujo5VlYUjAvKXV0b9DQzJ2dmI+OieHh3dra1kVEPk5NR6mppPb284OCfSopImNiXcHBvcfHwzY1LzMyK3x7dtXV0fDw7EFAOZaWkeXl4fn597CvqywrJFNSTLe2skdHQD8+N97d2mNjXeHh3o+Piru7tzAvKElIQoiIg8vLxzk4MW5uaPz8+bW0sN/f2+np5ldWUODg3EFAOsC/u0ZFPvb18lpZU42Nh/////7Hc1oAAAABYktHRIqFaHd2AAAACXBIWXMAAA8uAAAPLgEh0EwaAAAAB3RJTUUH6gEeEiU4Kf6EPwAAACV0RVh0ZGF0ZTpjcmVhdGUAMjAyNi0wMS0zMFQxODozNDo1MCswMDowMOXwHaoAAAAldEVYdGRhdGU6bW9kaWZ5ADIwMjUtMDktMTBUMDk6MTk6MTArMDA6MDAZCYFsAAAAKHRFWHRkYXRlOnRpbWVzdGFtcAAyMDI2LTAxLTMwVDE4OjM3OjU2KzAwOjAwS18K8AAAApNJREFUWMPtl+lXEzEUxSeBlAZIBkFrw2KhWAoWCl2gVVuX2iouCCqCCyCigICKu7jirvzXZlK6AJNZP+nhnn5pTvM7efck790qyr7+fwEuF9shrKmtrYHQ6XbkqfNi7K3zIEcIVN/QSCjhn8aGemS/eLXpQDMmQri55eAhe1ZA4Dvsx4xsi2F/qw9YrwOitvYOWt4uEPRIe5tVK1CgsytIyS7RYFdnwIoVQOk+GsJERzjU062YWQFBuLcPM6IrhvuOhY2tQJH+ASrZXrRioD8irwNEB4dilBiKxoYGo5I6YDyRHMamGk4m4pIyRlLp4yfMdTI1or8/k02dOn3mbMREuXM0m9EFqPkCO39hFAFoqIuXLudVGYAwdmUsYnhd0NVkwQDAbR6fuBaVXxd4/QY2BvAbN3lzCsoQYDpGzAD8xqVv3dZ/OeDOXWoO4HWwmdmcnhVz9/hFswDgiPn7C+qeOtCDh8wigFuxuLS86+XAR3ntnVoE8Jezsrr2eMcBnjxlNgBaP11/Nld1gLV10SisA/hvn3sqVYDV7UUbAPriZdlJsLxC7QO8o2WAukQdnKACQAuLzA0A5l6VWq0zAJqdJ24A4PUMdQWAbyrN2gkATKWpK0A0UTWsZIBMVgpAbyerxk1B0lR5W2f6ABiZqDoAk7V1Plg2sC4AjI1XlvCGdLDojDYBAPF35VWj0aboDNfiCd6XlkyGq7J3vGsA9OFjcYGP996wadLhAaOnEjA0QOBT8aulgCHqqIo4HPB5M6QdwHLEUXaELOr98vUb1orvsB6yhBW+VhHzqPf7dNB2zBNWqE0tPGjSHz9/US1o/rafuUXUxf4/zFnUVUphu+A4bCsi7m9tOY/7wgp3fzj29a/oL09sk0pvLkBgAAAAAElFTkSuQmCC" alt="Cursor"></button>
            <button class="wormhole-btn wormhole-btn-icon wormhole-btn-vscode" title="Open embedded VSCode"><img src="https://vscode.dev/static/stable/code-192.png" alt="VSCode"></button>
        `;
    }

    if (!html) return null;

    container.innerHTML = html;

    const termBtn = container.querySelector('.wormhole-btn-terminal');
    const cursorBtn = container.querySelector('.wormhole-btn-cursor');
    const vscodeBtn = container.querySelector('.wormhole-btn-vscode');

    if (termBtn) {
        termBtn.addEventListener('click', (e) => {
            e.preventDefault();
            e.stopPropagation();
            switchProject('terminal');
        });
    }

    if (cursorBtn) {
        cursorBtn.addEventListener('click', (e) => {
            e.preventDefault();
            e.stopPropagation();
            switchProject('editor');
        });
    }

    if (vscodeBtn) {
        vscodeBtn.addEventListener('click', (e) => {
            e.preventDefault();
            e.stopPropagation();
            toggleVSCode(info.name, vscodeBtn);
        });
    }

    return container;
}

async function toggleVSCode(projectName, vscodeBtn) {
    let container = document.querySelector('.wormhole-vscode-container');

    if (vscodeExpanded) {
        // Close
        if (container) {
            container.classList.remove('expanded');
        }
        vscodeBtn.classList.remove('active');
        vscodeBtn.style.display = '';
        vscodeExpanded = false;

        // If maximized, restore first
        if (vscodeMaximized) {
            const controlBtn = container?.querySelector('.wormhole-control-maximize');
            if (controlBtn) toggleMaximizeToolbar(controlBtn);
        }
    } else {
        // Open
        vscodeBtn.style.opacity = '0.5';
        vscodeBtn.disabled = true;

        try {
            const resp = await fetch(`${WORMHOLE_BASE}/project/vscode/${encodeURIComponent(projectName)}`);
            if (!resp.ok) {
                console.warn('[Wormhole] VSCode server failed:', await resp.text());
                vscodeBtn.style.opacity = '';
                vscodeBtn.disabled = false;
                return;
            }

            const data = await resp.json();

            if (!container) {
                container = createVSCodeContainer();
            }

            const iframe = container.querySelector('iframe');
            iframe.src = data.url;

            container.classList.add('expanded');
            vscodeBtn.classList.add('active');
            vscodeBtn.style.opacity = '';
            vscodeExpanded = true;

            // Also switch to the project (skip editor since we're showing embedded)
            fetch(`${WORMHOLE_BASE}/project/switch/${encodeURIComponent(projectName)}?skip-editor=true`);
        } catch (err) {
            console.warn('[Wormhole] VSCode error:', err.message);
            vscodeBtn.style.opacity = '';
            vscodeBtn.style.display = '';
        } finally {
            vscodeBtn.disabled = false;
        }
    }
}

function createVSCodeContainer() {
    const container = document.createElement('div');
    container.className = 'wormhole-vscode-container';
    container.innerHTML = `
        <iframe></iframe>
        <div class="wormhole-vscode-controls">
            <button class="wormhole-control-btn wormhole-control-maximize">Maximize</button>
            <button class="wormhole-control-btn wormhole-control-close">Close</button>
        </div>
    `;
    document.body.appendChild(container);

    const controlMaximize = container.querySelector('.wormhole-control-maximize');
    const controlClose = container.querySelector('.wormhole-control-close');

    controlMaximize.addEventListener('click', () => {
        toggleMaximizeToolbar(controlMaximize);
    });

    controlClose.addEventListener('click', () => {
        closeVSCode();
    });

    // ESC to restore from maximized
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && vscodeMaximized) {
            toggleMaximizeToolbar(container.querySelector('.wormhole-control-maximize'));
        }
    });

    return container;
}

function toggleMaximizeToolbar(btn) {
    const container = document.querySelector('.wormhole-vscode-container');
    if (!container) return;

    const closeBtn = container.querySelector('.wormhole-control-close');

    if (vscodeMaximized) {
        container.classList.remove('maximized');
        btn.textContent = 'Maximize';
        document.body.style.overflow = '';
        vscodeMaximized = false;
        if (closeBtn) closeBtn.style.display = '';
    } else {
        container.classList.add('maximized');
        btn.textContent = 'Restore';
        document.body.style.overflow = 'hidden';
        vscodeMaximized = true;
        if (closeBtn) closeBtn.style.display = 'none';
    }
}

function closeVSCode() {
    const container = document.querySelector('.wormhole-vscode-container');
    if (container) {
        container.classList.remove('expanded', 'maximized');
        const iframe = container.querySelector('iframe');
        if (iframe) iframe.src = '';
        const closeBtn = container.querySelector('.wormhole-control-close');
        if (closeBtn) closeBtn.style.display = '';
    }
    document.body.style.overflow = '';
    vscodeExpanded = false;
    vscodeMaximized = false;

    // Restore header VSCode button
    const vscodeBtn = document.querySelector('.wormhole-btn-vscode');
    if (vscodeBtn) {
        vscodeBtn.classList.remove('active');
        vscodeBtn.style.display = '';
    }
}

async function switchProject(landIn) {
    try {
        const info = await getDescribe();
        if (!info || !info.name) {
            console.warn('[Wormhole] No project/task found');
            return;
        }

        const params = new URLSearchParams({ 'land-in': landIn });
        if (landIn === 'terminal') {
            params.set('skip-editor', 'true');
            params.set('focus-terminal', 'true');
        }

        const switchResp = await fetch(
            `${WORMHOLE_BASE}/project/switch/${encodeURIComponent(info.name)}?${params}`
        );

        if (!switchResp.ok) {
            console.warn('[Wormhole] switch failed:', await switchResp.text());
        } else {
            console.log('[Wormhole] Switched to', info.name);
        }
    } catch (err) {
        console.warn('[Wormhole] Error:', err.message);
    }
}

function injectStyles() {
    if (document.getElementById('wormhole-styles')) return;

    const style = document.createElement('style');
    style.id = 'wormhole-styles';
    style.textContent = `
        .wormhole-buttons {
            display: inline-flex;
            gap: 0.5rem;
            margin-left: 1rem;
            vertical-align: middle;
            align-items: center;
        }
        .wormhole-btn {
            font-family: "SF Mono", "Menlo", "Monaco", monospace;
            font-size: 0.75rem;
            padding: 0.25rem 0.75rem;
            border: 1px solid #999;
            background: #fff;
            color: #666;
            cursor: pointer;
            transition: background 0.1s, color 0.1s, opacity 0.1s;
            text-decoration: none;
        }
        .wormhole-btn:hover {
            background: #666;
            color: #fff;
        }
        .wormhole-btn:disabled {
            opacity: 0.5;
            cursor: not-allowed;
        }
        .wormhole-btn-icon {
            padding: 0.25rem;
            border: none;
            background: transparent;
            opacity: 0.7;
        }
        .wormhole-btn-icon:hover {
            background: transparent;
            opacity: 1;
        }
        .wormhole-btn-icon img {
            width: 20px;
            height: 20px;
            display: block;
        }
        .wormhole-btn-vscode.active {
            opacity: 1;
            background: rgba(0, 102, 204, 0.1);
            border-radius: 4px;
        }
        .wormhole-link {
            font-family: "SF Mono", "Menlo", "Monaco", monospace;
            font-size: 0.75rem;
            text-decoration: none;
            font-weight: 500;
        }
        .wormhole-link:hover {
            text-decoration: underline;
        }
        .wormhole-link-jira {
            color: #0052cc;
        }
        .wormhole-link-github {
            color: #238636;
        }
        .wormhole-vscode-container {
            display: none;
            position: fixed;
            bottom: 0;
            left: 0;
            right: 0;
            height: 50vh;
            background: #fff;
            border-top: 2px solid #0066cc;
            z-index: 9999;
            box-shadow: 0 -4px 20px rgba(0,0,0,0.2);
        }
        .wormhole-vscode-container.expanded {
            display: block;
        }
        .wormhole-vscode-container iframe {
            width: 100%;
            height: 100%;
            border: none;
        }
        .wormhole-vscode-controls {
            position: absolute;
            top: 8px;
            right: 8px;
            display: flex;
            gap: 0.5rem;
            z-index: 10;
        }
        .wormhole-control-btn {
            font-family: "SF Mono", "Menlo", "Monaco", monospace;
            font-size: 0.7rem;
            padding: 0.3rem 0.7rem;
            border: 1px solid rgba(255,255,255,0.3);
            background: rgba(30, 30, 30, 0.85);
            color: #fff;
            cursor: pointer;
            backdrop-filter: blur(4px);
            border-radius: 3px;
        }
        .wormhole-control-btn:hover {
            background: rgba(60, 60, 60, 0.95);
            border-color: rgba(255,255,255,0.5);
        }
        .wormhole-vscode-container.maximized {
            top: 0;
            height: 100vh;
        }
    `;
    document.head.appendChild(style);
}

function getTargetSelectors() {
    if (isGitHubPage()) {
        return [
            '.gh-header-title',
            '.gh-header-actions',
            '.gh-header-meta',
            '#partial-discussion-header',
            '.AppHeader-context-full',
        ];
    } else if (isJiraPage()) {
        return [
            // Breadcrumbs area (preferred - above title)
            '[data-testid="issue.views.issue-base.foundation.breadcrumbs.breadcrumb-current-issue-container"]',
            '[data-test-id="issue.views.issue-base.foundation.breadcrumbs.current-issue.item"]',
            '[data-testid="issue.views.issue-base.foundation.breadcrumbs.parent-issue.item"]',
            // Board view modal selectors
            '[data-testid="issue.views.issue-base.foundation.summary.heading"]',
            '[data-testid="issue-details-panel-header"]',
            // Browse page selectors
            '[data-testid="issue-header"]',
            '#jira-issue-header',
        ];
    }
    return [];
}

function shouldInject() {
    if (isGitHubPage()) {
        const path = window.location.pathname;
        if (!path.match(/^\/[^/]+\/[^/]+/)) return false;
        if (path.match(/^\/(settings|notifications|new|login|signup)/)) return false;
        return true;
    } else if (isJiraPage()) {
        // /browse/ACT-108 or board view with ?selectedIssue=ACT-108
        return window.location.pathname.includes('/browse/') ||
               window.location.search.includes('selectedIssue=');
    }
    return false;
}

let retryCount = 0;

async function injectButtons() {
    // Prevent concurrent injections
    if (injecting) return;
    if (document.querySelector('.wormhole-buttons')) return;
    if (!shouldInject()) return;

    injecting = true;

    try {
        injectStyles();

        const selectors = getTargetSelectors();
        let targetElement = null;
        for (const sel of selectors) {
            targetElement = document.querySelector(sel);
            if (targetElement) break;
        }

        if (targetElement) {
            // Double-check no buttons were added while we waited
            if (document.querySelector('.wormhole-buttons')) return;

            const info = await getDescribe();

            // Triple-check after async call
            if (document.querySelector('.wormhole-buttons')) return;

            const buttons = createButtons(info);
            if (buttons) {
                targetElement.appendChild(buttons);
            }
            retryCount = 0;
        } else {
            // Retry - pages load content dynamically
            if (retryCount++ < 15) {
                setTimeout(injectButtons, 300);
            }
        }
    } finally {
        injecting = false;
    }
}

// Run on page load
injectButtons();

// Re-run on navigation (SPA routing) - debounced
let lastUrl = window.location.href;
let debounceTimer = null;

const observer = new MutationObserver(() => {
    if (window.location.href !== lastUrl) {
        lastUrl = window.location.href;
        retryCount = 0;
        cachedDescribe = null;
        cachedUrl = null;
        vscodeExpanded = false;
        vscodeMaximized = false;
        document.querySelectorAll('.wormhole-buttons').forEach(el => el.remove());
        document.querySelectorAll('.wormhole-vscode-container').forEach(el => el.remove());
        document.body.style.overflow = '';
        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(injectButtons, 100);
    } else if (!document.querySelector('.wormhole-buttons') && shouldInject() && !injecting) {
        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(injectButtons, 200);
    }
});
observer.observe(document.body, { childList: true, subtree: true });
