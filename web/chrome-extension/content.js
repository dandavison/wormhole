// Wormhole GitHub Integration
// Adds Terminal and Cursor buttons to GitHub pages

const WORMHOLE_PORT = 7117;
const WORMHOLE_BASE = `http://localhost:${WORMHOLE_PORT}`;

function createButtons() {
    const container = document.createElement('div');
    container.className = 'wormhole-buttons';
    container.innerHTML = `
        <button class="wormhole-btn wormhole-btn-terminal" title="Open in Terminal">Terminal</button>
        <button class="wormhole-btn wormhole-btn-cursor" title="Open in Cursor">Cursor</button>
    `;

    container.querySelector('.wormhole-btn-terminal').addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        switchProject('terminal');
    });

    container.querySelector('.wormhole-btn-cursor').addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        switchProject('editor');
    });

    return container;
}

async function switchProject(landIn) {
    try {
        // Ask wormhole to describe the current URL
        const describeResp = await fetch(`${WORMHOLE_BASE}/project/describe`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ url: window.location.href })
        });

        if (!describeResp.ok) {
            console.warn('[Wormhole] describe failed:', await describeResp.text());
            return;
        }

        const info = await describeResp.json();
        console.log('[Wormhole] describe:', info);

        if (!info.name) {
            console.warn('[Wormhole] No project/task found');
            return;
        }

        // Switch to the project/task
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
    `;
    document.head.appendChild(style);
}

function injectButtons() {
    if (document.querySelector('.wormhole-buttons')) return;

    // Only inject on github.com repo/PR pages
    const path = window.location.pathname;
    if (!path.match(/^\/[^/]+\/[^/]+/)) return;
    if (path.match(/^\/(settings|notifications|new|login|signup)/)) return;

    injectStyles();

    // Find a place to insert buttons - try multiple selectors
    const selectors = [
        '.gh-header-title',
        '.gh-header-actions',
        '.gh-header-meta',
        '#partial-discussion-header',
        '.AppHeader-context-full',
    ];

    let targetElement = null;
    for (const sel of selectors) {
        targetElement = document.querySelector(sel);
        if (targetElement) break;
    }

    if (targetElement) {
        const buttons = createButtons();
        targetElement.appendChild(buttons);
    } else {
        // Retry - GitHub loads content dynamically
        if (!injectButtons.retryCount) injectButtons.retryCount = 0;
        if (injectButtons.retryCount++ < 10) {
            setTimeout(injectButtons, 300);
        }
    }
}

// Run on page load
injectButtons();

// Re-run on navigation (GitHub uses client-side routing)
let lastPath = window.location.pathname;
const observer = new MutationObserver(() => {
    if (window.location.pathname !== lastPath) {
        lastPath = window.location.pathname;
        injectButtons.retryCount = 0;
        document.querySelectorAll('.wormhole-buttons').forEach(el => el.remove());
        setTimeout(injectButtons, 100);
    } else if (!document.querySelector('.wormhole-buttons')) {
        // Buttons disappeared (GitHub re-rendered), try again
        setTimeout(injectButtons, 100);
    }
});
observer.observe(document.body, { childList: true, subtree: true });
