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

    // Only show Terminal/Cursor/VSCode buttons if we have a task/project to switch to
    if (info?.name && info?.kind) {
        html += `
            <button class="wormhole-btn wormhole-btn-terminal" title="Open in Terminal">Terminal</button>
            <button class="wormhole-btn wormhole-btn-cursor" title="Open in Cursor">Cursor</button>
            <button class="wormhole-btn wormhole-btn-vscode" title="Open embedded VSCode">VSCode</button>
            <button class="wormhole-btn wormhole-btn-maximize" title="Maximize VSCode" style="display:none;">Maximize</button>
        `;
    }

    // Add GitHub link on JIRA pages (show as "repo#123")
    if (info?.github_url && info?.github_label) {
        html += `<a class="wormhole-link wormhole-link-github" href="${info.github_url}" title="Open GitHub PR">${info.github_label}</a>`;
    }

    // Add JIRA link on GitHub pages (show as "ACT-123")
    if (info?.jira_url && info?.jira_key && isGitHubPage()) {
        html += `<a class="wormhole-link wormhole-link-jira" href="${info.jira_url}" title="Open JIRA">${info.jira_key}</a>`;
    }

    if (!html) return null;

    container.innerHTML = html;

    const termBtn = container.querySelector('.wormhole-btn-terminal');
    const cursorBtn = container.querySelector('.wormhole-btn-cursor');
    const vscodeBtn = container.querySelector('.wormhole-btn-vscode');
    const maximizeBtn = container.querySelector('.wormhole-btn-maximize');

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
            toggleVSCode(info.name, vscodeBtn, maximizeBtn);
        });
    }

    if (maximizeBtn) {
        maximizeBtn.addEventListener('click', (e) => {
            e.preventDefault();
            e.stopPropagation();
            toggleMaximize(maximizeBtn);
        });
    }

    return container;
}

async function toggleVSCode(projectName, vscodeBtn, maximizeBtn) {
    let container = document.querySelector('.wormhole-vscode-container');

    if (vscodeExpanded) {
        // Close
        if (container) {
            container.classList.remove('expanded');
        }
        vscodeBtn.textContent = 'VSCode';
        vscodeBtn.classList.remove('active');
        maximizeBtn.style.display = 'none';
        vscodeExpanded = false;

        // If maximized, restore first
        if (vscodeMaximized) {
            toggleMaximize(maximizeBtn);
        }
    } else {
        // Open
        vscodeBtn.textContent = 'Loading...';
        vscodeBtn.disabled = true;

        try {
            const resp = await fetch(`${WORMHOLE_BASE}/project/vscode/${encodeURIComponent(projectName)}`);
            if (!resp.ok) {
                console.warn('[Wormhole] VSCode server failed:', await resp.text());
                vscodeBtn.textContent = 'VSCode';
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
            vscodeBtn.textContent = 'Close';
            vscodeBtn.classList.add('active');
            maximizeBtn.style.display = 'inline-block';
            vscodeExpanded = true;

            // Also switch to the project (skip editor since we're showing embedded)
            fetch(`${WORMHOLE_BASE}/project/switch/${encodeURIComponent(projectName)}?skip-editor=true`);
        } catch (err) {
            console.warn('[Wormhole] VSCode error:', err.message);
            vscodeBtn.textContent = 'VSCode';
        } finally {
            vscodeBtn.disabled = false;
        }
    }
}

function toggleMaximize(maximizeBtn) {
    const container = document.querySelector('.wormhole-vscode-container');
    if (!container) return;

    if (vscodeMaximized) {
        container.classList.remove('maximized');
        maximizeBtn.textContent = 'Maximize';
        document.body.style.overflow = '';
        vscodeMaximized = false;
    } else {
        container.classList.add('maximized');
        maximizeBtn.textContent = 'Restore';
        document.body.style.overflow = 'hidden';
        vscodeMaximized = true;
    }
}

function createVSCodeContainer() {
    const container = document.createElement('div');
    container.className = 'wormhole-vscode-container';
    container.innerHTML = '<iframe></iframe>';
    document.body.appendChild(container);

    // ESC to restore from maximized
    document.addEventListener('keydown', (e) => {
        if (e.key === 'Escape' && vscodeMaximized) {
            const maximizeBtn = document.querySelector('.wormhole-btn-maximize');
            if (maximizeBtn) {
                toggleMaximize(maximizeBtn);
            }
        }
    });

    return container;
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
            transition: background 0.1s, color 0.1s;
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
        .wormhole-btn-cursor, .wormhole-btn-vscode {
            border-color: #0066cc;
            color: #0066cc;
        }
        .wormhole-btn-cursor:hover, .wormhole-btn-vscode:hover {
            background: #0066cc;
            color: #fff;
        }
        .wormhole-btn-vscode.active {
            background: #0066cc;
            color: #fff;
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
            // Board view modal selectors
            '[data-testid="issue.views.issue-base.foundation.summary.heading"]',
            '[data-testid="issue.views.issue-details.issue-layout.visible-when-published"]',
            '[data-testid="issue-details-panel-header"]',
            // Browse page selectors
            '[data-testid="issue-header"]',
            '#jira-issue-header',
            '#summary-val',
            '.issue-header-content',
            '[data-test-id="issue.views.issue-base.foundation.breadcrumbs.current-issue.item"]',
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
