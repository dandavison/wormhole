// Wormhole GitHub/JIRA Integration
// Adds Terminal, Cursor, and cross-linking buttons to GitHub and JIRA pages

const WORMHOLE_PORT = 7117;
const WORMHOLE_BASE = `http://localhost:${WORMHOLE_PORT}`;

// Cache describe result for current page
let cachedDescribe = null;
let cachedUrl = null;

// Prevent concurrent injection
let injecting = false;

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

    // Only show Terminal/Cursor buttons if we have a task/project to switch to
    if (info?.name && info?.kind) {
        html += `
            <button class="wormhole-btn wormhole-btn-terminal" title="Open in Terminal">Terminal</button>
            <button class="wormhole-btn wormhole-btn-cursor" title="Open in Cursor">Cursor</button>
        `;
    }

    // Add GitHub link on JIRA pages
    if (info?.github_url) {
        html += `<a class="wormhole-btn wormhole-btn-github" href="${info.github_url}" title="Open GitHub PR">GitHub</a>`;
    }

    // Add JIRA link on GitHub pages
    if (info?.jira_url && isGitHubPage()) {
        html += `<a class="wormhole-btn wormhole-btn-jira" href="${info.jira_url}" title="Open JIRA">JIRA</a>`;
    }

    if (!html) return null;

    container.innerHTML = html;

    const termBtn = container.querySelector('.wormhole-btn-terminal');
    const cursorBtn = container.querySelector('.wormhole-btn-cursor');

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
        .wormhole-btn-cursor {
            border-color: #0066cc;
            color: #0066cc;
        }
        .wormhole-btn-cursor:hover {
            background: #0066cc;
            color: #fff;
        }
        .wormhole-btn-jira {
            border-color: #0052cc;
            color: #0052cc;
        }
        .wormhole-btn-jira:hover {
            background: #0052cc;
            color: #fff;
        }
        .wormhole-btn-github {
            border-color: #238636;
            color: #238636;
        }
        .wormhole-btn-github:hover {
            background: #238636;
            color: #fff;
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
        document.querySelectorAll('.wormhole-buttons').forEach(el => el.remove());
        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(injectButtons, 100);
    } else if (!document.querySelector('.wormhole-buttons') && shouldInject() && !injecting) {
        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(injectButtons, 200);
    }
});
observer.observe(document.body, { childList: true, subtree: true });
