// Wormhole GitHub Integration
// Adds Terminal and Cursor buttons to GitHub PR and repo pages

const WORMHOLE_PORT = 7117;
const WORMHOLE_BASE = `http://localhost:${WORMHOLE_PORT}`;

function getPageInfo() {
    const path = window.location.pathname;

    // PR page: /owner/repo/pull/123
    const prMatch = path.match(/^\/([^/]+)\/([^/]+)\/pull\/(\d+)/);
    if (prMatch) {
        return { type: 'pr', owner: prMatch[1], repo: prMatch[2], prNumber: prMatch[3] };
    }

    // Other repo pages: /owner/repo/...
    const repoMatch = path.match(/^\/([^/]+)\/([^/]+)/);
    if (repoMatch && !['settings', 'notifications', 'new'].includes(repoMatch[2])) {
        return { type: 'repo', owner: repoMatch[1], repo: repoMatch[2] };
    }

    return null;
}

function getBranchName() {
    // PR page: look for the branch name in the head ref
    // GitHub shows it as "user:branch" or just "branch" in the PR header

    // Try various selectors - GitHub's DOM varies by page/tab
    const selectors = [
        '.head-ref a span',
        '.head-ref span',
        '.commit-ref.head-ref',
        '[data-testid="head-ref-name"]',
        '.gh-header-meta .commit-ref:last-child',
    ];

    for (const selector of selectors) {
        const el = document.querySelector(selector);
        if (el) {
            const text = el.textContent.trim();
            if (text) {
                // Handle "user:branch" format
                return text.includes(':') ? text.split(':')[1] : text;
            }
        }
    }

    return null;
}

function createButtons(projectName) {
    const container = document.createElement('div');
    container.className = 'wormhole-buttons';
    container.innerHTML = `
        <button class="wormhole-btn wormhole-btn-terminal" title="Open in Terminal">Terminal</button>
        <button class="wormhole-btn wormhole-btn-cursor" title="Open in Cursor">Cursor</button>
    `;

    container.querySelector('.wormhole-btn-terminal').addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        switchProject(projectName, 'terminal');
    });

    container.querySelector('.wormhole-btn-cursor').addEventListener('click', (e) => {
        e.preventDefault();
        e.stopPropagation();
        switchProject(projectName, 'editor');
    });

    return container;
}

async function switchProject(name, landIn) {
    const params = new URLSearchParams({
        'land-in': landIn,
        'skip-editor': landIn === 'terminal' ? 'true' : 'false',
        'focus-terminal': landIn === 'terminal' ? 'true' : 'false'
    });

    try {
        const response = await fetch(`${WORMHOLE_BASE}/project/switch/${name}?${params}`);
        if (!response.ok) {
            console.warn('Wormhole switch failed:', await response.text());
        }
    } catch (err) {
        console.warn('Wormhole server not reachable:', err.message);
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
    // Don't inject twice
    if (document.querySelector('.wormhole-buttons')) return;

    const pageInfo = getPageInfo();
    if (!pageInfo) return;

    injectStyles();

    let projectName;
    let targetElement;

    if (pageInfo.type === 'pr') {
        // For PRs, use branch name as project/task name
        projectName = getBranchName();
        if (!projectName) {
            // Retry after a short delay (GitHub loads content dynamically)
            setTimeout(injectButtons, 500);
            return;
        }

        // Insert in the PR header area - try multiple selectors
        targetElement = document.querySelector('.gh-header-title')
            || document.querySelector('.gh-header-actions')
            || document.querySelector('[data-testid="issue-title"]')?.parentElement
            || document.querySelector('.gh-header-meta');
    } else {
        // For repo pages, use repo name
        projectName = pageInfo.repo;

        // Insert in the repo header area
        targetElement = document.querySelector('.AppHeader-context-full');
        if (!targetElement) {
            targetElement = document.querySelector('[data-testid="repository-title-link"]')?.parentElement;
        }
    }

    if (targetElement && projectName) {
        const buttons = createButtons(projectName);
        targetElement.appendChild(buttons);
    } else if (projectName) {
        // Target element not found, retry shortly
        setTimeout(injectButtons, 500);
    }
}

// Run on page load
injectButtons();

// Re-run on navigation and when GitHub re-renders (tab switches, etc.)
let lastPath = window.location.pathname;
let retryCount = 0;
const observer = new MutationObserver(() => {
    const currentPath = window.location.pathname;
    const buttonsExist = document.querySelector('.wormhole-buttons');

    if (currentPath !== lastPath) {
        lastPath = currentPath;
        retryCount = 0;
        document.querySelectorAll('.wormhole-buttons').forEach(el => el.remove());
        setTimeout(injectButtons, 100);
    } else if (!buttonsExist && retryCount < 5) {
        // Buttons missing (GitHub re-rendered), try to inject again
        retryCount++;
        setTimeout(injectButtons, 200);
    }
});
observer.observe(document.body, { childList: true, subtree: true });
