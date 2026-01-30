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
            <button class="wormhole-btn wormhole-btn-icon wormhole-btn-terminal" title="Open in Terminal"><img src="data:image/png;base64,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" alt="Terminal"></button>
            <button class="wormhole-btn wormhole-btn-icon wormhole-btn-cursor" title="Open in Cursor"><img src="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAEAAAABACAMAAACdt4HsAAAAIGNIUk0AAHomAACAhAAA+gAAAIDoAAB1MAAA6mAAADqYAAAXcJy6UTwAAAGhUExURff39Pn59vj49dPTz42Mh/r69+Pj4JOTjj8+OCMiG+/v662tqFFQSiYlHiQjHPX18sTEwGdmYCsqJGdmYcXEwPb28tjY1IB/ejU0LSUkHYCAeufn45qalUNCPENDPPHx7rSzr1ZWUCcmH8rKxm1tZy4tJm5tZ9zc2YeHgTg3MYeHgurq56GhnEhHQfPz8Lq6tlxcVignIN3d2XV1by8uJyEgGICAeyEgGT08NVJRS09OSDs6M/X18WloY1ZVT76+uurq5uzs6O3t6qSjn2ppYyIhGjo5MomIg97e2vPy73JxbCAfGElIQaKinba2sikoIV1cVru6tujo5VlYUjAvKXV0b9DQzJ2dmI+OieHh3dra1kVEPk5NR6mppPb284OCfSopImNiXcHBvcfHwzY1LzMyK3x7dtXV0fDw7EFAOZaWkeXl4fn597CvqywrJFNSTLe2skdHQD8+N97d2mNjXeHh3o+Piru7tzAvKElIQoiIg8vLxzk4MW5uaPz8+bW0sN/f2+np5ldWUODg3EFAOsC/u0ZFPvb18lpZU42Nh/////7Hc1oAAAABYktHRIqFaHd2AAAACXBIWXMAAA8uAAAPLgEh0EwaAAAAB3RJTUUH6gEeEiU4Kf6EPwAAACV0RVh0ZGF0ZTpjcmVhdGUAMjAyNi0wMS0zMFQxODozNDo1MCswMDowMOXwHaoAAAAldEVYdGRhdGU6bW9kaWZ5ADIwMjUtMDktMTBUMDk6MTk6MTArMDA6MDAZCYFsAAAAKHRFWHRkYXRlOnRpbWVzdGFtcAAyMDI2LTAxLTMwVDE4OjM3OjU2KzAwOjAwS18K8AAAApNJREFUWMPtl+lXEzEUxSeBlAZIBkFrw2KhWAoWCl2gVVuX2iouCCqCCyCigICKu7jirvzXZlK6AJNZP+nhnn5pTvM7efck790qyr7+fwEuF9shrKmtrYHQ6XbkqfNi7K3zIEcIVN/QSCjhn8aGemS/eLXpQDMmQri55eAhe1ZA4Dvsx4xsi2F/qw9YrwOitvYOWt4uEPRIe5tVK1CgsytIyS7RYFdnwIoVQOk+GsJERzjU062YWQFBuLcPM6IrhvuOhY2tQJH+ASrZXrRioD8irwNEB4dilBiKxoYGo5I6YDyRHMamGk4m4pIyRlLp4yfMdTI1or8/k02dOn3mbMREuXM0m9EFqPkCO39hFAFoqIuXLudVGYAwdmUsYnhd0NVkwQDAbR6fuBaVXxd4/QY2BvAbN3lzCsoQYDpGzAD8xqVv3dZ/OeDOXWoO4HWwmdmcnhVz9/hFswDgiPn7C+qeOtCDh8wigFuxuLS86+XAR3ntnVoE8Jezsrr2eMcBnjxlNgBaP11/Nld1gLV10SisA/hvn3sqVYDV7UUbAPriZdlJsLxC7QO8o2WAukQdnKACQAuLzA0A5l6VWq0zAJqdJ24A4PUMdQWAbyrN2gkATKWpK0A0UTWsZIBMVgpAbyerxk1B0lR5W2f6ABiZqDoAk7V1Plg2sC4AjI1XlvCGdLDojDYBAPF35VWj0aboDNfiCd6XlkyGq7J3vGsA9OFjcYGP996wadLhAaOnEjA0QOBT8aulgCHqqIo4HPB5M6QdwHLEUXaELOr98vUb1orvsB6yhBW+VhHzqPf7dNB2zBNWqE0tPGjSHz9/US1o/rafuUXUxf4/zFnUVUphu+A4bCsi7m9tOY/7wgp3fzj29a/oL09sk0pvLkBgAAAAAElFTkSuQmCC" alt="Cursor"></button>
            <button class="wormhole-btn wormhole-btn-icon wormhole-btn-vscode" title="Open embedded VSCode"><img src="data:image/png;base64,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" alt="VSCode"></button>
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
