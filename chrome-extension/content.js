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

// Agent panel state
let agentPollController = null;
let agentPanelVisible = false;
let agentBatchId = null;

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

// Build the wormhole button bar. Used identically for PR pages, repo/JIRA pages,
// and GitHub PR-inbox rows. `info` is the /project/describe result (may be null
// when we haven't resolved a task yet); `ref` is a GitHub PR URL that can be
// opened on demand (create-if-missing) when no task exists yet.
function createButtons(info, ref) {
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

    // Terminal button when we have a task/project, or a PR we can open on demand
    if ((info?.name && info?.kind) || isPrUrl(ref)) {
        html += `
            <button class="wormhole-btn wormhole-btn-icon wormhole-btn-terminal" title="Open in Terminal"><img src="data:image/png;base64,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" alt="Terminal"></button>
        `;
    }

    if (!html) return null;

    container.innerHTML = html;

    const termBtn = container.querySelector('.wormhole-btn-terminal');
    if (termBtn) {
        termBtn.addEventListener('click', (e) => {
            e.preventDefault();
            e.stopPropagation();
            openTarget(ref, info, 'terminal-only', termBtn);
        });
    }

    return container;
}

function updateAgentLight(status) {
    const light = document.querySelector('.wormhole-agent-status');
    if (!light) return;
    light.classList.remove('wormhole-agent-idle', 'wormhole-agent-running', 'wormhole-agent-failed');
    if (status === 'running') {
        light.classList.add('wormhole-agent-running');
        light.title = 'Agent running';
    } else if (status === 'failed') {
        light.classList.add('wormhole-agent-failed');
        light.title = 'Agent failed';
    } else {
        light.classList.add('wormhole-agent-idle');
        light.title = 'Agent idle';
    }
}

// -- Agent stream panel --

function createAgentPanel() {
    let panel = document.querySelector('.wormhole-agent-panel');
    if (panel) return panel;
    panel = document.createElement('div');
    panel.className = 'wormhole-agent-panel';
    panel.innerHTML = `
        <div class="wormhole-agent-panel-header">
            <span class="wormhole-agent-panel-title">Agent</span>
            <button class="wormhole-agent-panel-close">\u00d7</button>
        </div>
        <div class="wormhole-agent-panel-body"></div>
        <div class="wormhole-agent-panel-footer"></div>
    `;
    document.body.appendChild(panel);
    panel.querySelector('.wormhole-agent-panel-close').addEventListener('click', () => {
        panel.classList.remove('visible');
        agentPanelVisible = false;
    });
    return panel;
}

function showAgentPanel() {
    const panel = createAgentPanel();
    panel.querySelector('.wormhole-agent-panel-body').textContent = '';
    panel.querySelector('.wormhole-agent-panel-footer').textContent = '';
    panel.classList.add('visible');
    agentPanelVisible = true;
}

function closeAgentPanel() {
    const panel = document.querySelector('.wormhole-agent-panel');
    if (panel) {
        panel.classList.remove('visible');
        panel.remove();
    }
    agentPanelVisible = false;
}

function appendToPanel(html, cls) {
    const body = document.querySelector('.wormhole-agent-panel-body');
    if (!body) return;
    if (cls) {
        const span = document.createElement('span');
        span.className = cls;
        span.innerHTML = html;
        body.appendChild(span);
    } else {
        body.insertAdjacentHTML('beforeend', html);
    }
    body.scrollTop = body.scrollHeight;
}

function setPanelFooter(text) {
    const footer = document.querySelector('.wormhole-agent-panel-footer');
    if (footer) footer.textContent = text;
}

// -- Stream-json parsers --

function createStreamParser(agent) {
    if (agent === 'claude') return createClaudeStreamParser();
    return createCursorStreamParser();
}

function createClaudeStreamParser() {
    let remainder = '';
    let toolJsonBuf = '';
    let inToolBlock = false;
    let toolName = '';

    function processLine(line) {
        if (!line.trim()) return;
        let obj;
        try { obj = JSON.parse(line); } catch { return; }

        if (obj.type === 'system' && obj.subtype === 'init') {
            appendToPanel(escapeHtml(obj.model || 'claude') + '\n', 'wormhole-stream-meta');
            return;
        }

        if (obj.type === 'stream_event') {
            const ev = obj.event;
            if (!ev) return;

            if (ev.type === 'content_block_start' && ev.content_block?.type === 'tool_use') {
                inToolBlock = true;
                toolJsonBuf = '';
                toolName = ev.content_block.name || 'tool';
                appendToPanel('\n' + escapeHtml(toolName) + ' ', 'wormhole-stream-tool-name');
                return;
            }

            if (ev.type === 'content_block_delta') {
                if (ev.delta?.type === 'text_delta') {
                    appendToPanel(escapeHtml(ev.delta.text));
                } else if (ev.delta?.type === 'input_json_delta' && inToolBlock) {
                    toolJsonBuf += ev.delta.partial_json;
                }
                return;
            }

            if (ev.type === 'content_block_stop') {
                if (inToolBlock) {
                    try {
                        const input = JSON.parse(toolJsonBuf);
                        const cmd = input.command || input.input || toolJsonBuf;
                        appendToPanel(escapeHtml(cmd) + '\n', 'wormhole-stream-tool-input');
                    } catch {
                        appendToPanel(escapeHtml(toolJsonBuf) + '\n', 'wormhole-stream-tool-input');
                    }
                    inToolBlock = false;
                    toolJsonBuf = '';
                }
                return;
            }
            return;
        }

        if (obj.type === 'result') {
            const cost = obj.total_cost_usd != null ? `$${obj.total_cost_usd.toFixed(2)}` : '';
            const dur = obj.duration_ms != null ? `${(obj.duration_ms / 1000).toFixed(0)}s` : '';
            const status = obj.subtype === 'success' ? 'done' : 'error';
            setPanelFooter([status, dur, cost].filter(Boolean).join(' \u00b7 '));
            return;
        }
    }

    return function feed(chunk) {
        const text = remainder + chunk;
        const lines = text.split('\n');
        remainder = lines.pop();
        for (const line of lines) processLine(line);
    };
}

function createCursorStreamParser() {
    let remainder = '';
    let shownTextLen = 0;

    function processLine(line) {
        if (!line.trim()) return;
        let obj;
        try { obj = JSON.parse(line); } catch { return; }

        if (obj.type === 'system' && obj.subtype === 'init') {
            appendToPanel(escapeHtml(obj.model || 'cursor') + '\n', 'wormhole-stream-meta');
            return;
        }

        if (obj.type === 'assistant') {
            const blocks = obj.message?.content;
            if (!Array.isArray(blocks)) return;
            let fullText = '';
            for (const b of blocks) {
                if (b.type === 'text' && b.text) fullText += b.text;
            }
            if (fullText.length > shownTextLen) {
                appendToPanel(escapeHtml(fullText.slice(shownTextLen)));
                shownTextLen = fullText.length;
            }
            return;
        }

        if (obj.type === 'tool_call') {
            const tc = obj.tool_call || {};
            const toolKey = Object.keys(tc)[0] || '';
            const toolLabel = toolKey.replace(/ToolCall$/, '');
            if (obj.subtype === 'started') {
                appendToPanel('\n' + escapeHtml(toolLabel) + ' ', 'wormhole-stream-tool-name');
                const args = tc[toolKey]?.args;
                if (args) {
                    const summary = args.command || args.pattern || args.path || args.query || args.glob || args.globPattern || '';
                    if (summary) {
                        appendToPanel(escapeHtml(String(summary)) + '\n', 'wormhole-stream-tool-input');
                    }
                }
            }
            return;
        }

        if (obj.type === 'result') {
            const cost = obj.total_cost_usd != null ? `$${obj.total_cost_usd.toFixed(2)}` : '';
            const dur = obj.duration_ms != null ? `${(obj.duration_ms / 1000).toFixed(0)}s` : '';
            const status = obj.subtype === 'success' ? 'done' : 'error';
            setPanelFooter([status, dur, cost].filter(Boolean).join(' \u00b7 '));
            return;
        }
    }

    return function feed(chunk) {
        const text = remainder + chunk;
        const lines = text.split('\n');
        remainder = lines.pop();
        for (const line of lines) processLine(line);
    };
}

function escapeHtml(s) {
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

// -- Agent output polling --

async function pollAgentOutput(batchId, agent) {
    if (agentPollController) agentPollController.abort();
    agentPollController = new AbortController();
    const signal = agentPollController.signal;

    const parser = createStreamParser(agent);
    let offset = 0;

    updateAgentLight('running');
    while (!signal.aborted) {
        try {
            const resp = await fetch(
                `${WORMHOLE_BASE}/batch/${batchId}/output?run=0&offset=${offset}`,
                { signal }
            );
            if (!resp.ok || signal.aborted) break;
            const data = await resp.json();
            if (data.content) {
                parser(data.content);
            }
            offset = data.offset;
            if (data.done) {
                // Check final status
                const statusResp = await fetch(
                    `${WORMHOLE_BASE}/batch/${batchId}`,
                    { signal }
                );
                if (statusResp.ok) {
                    const batch = await statusResp.json();
                    const run = batch.runs?.[0];
                    updateAgentLight(run?.status === 'failed' ? 'failed' : 'idle');
                }
                return;
            }
            await new Promise(r => setTimeout(r, 300));
        } catch (err) {
            if (err.name === 'AbortError') return;
            console.warn('[Wormhole] agent poll error:', err.message);
            return;
        }
    }
}

async function notifyAgent(info) {
    const agentSelect = document.querySelector('.wormhole-agent-select');
    const agent = agentSelect ? agentSelect.value : undefined;
    try {
        updateAgentLight('running');
        const resp = await fetch(`${WORMHOLE_BASE}/task/notify-agent`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ task: info.name, agent })
        });
        if (resp.ok) {
            const data = await resp.json();
            if (data.status === 'interactive') {
                updateAgentLight('idle');
            } else {
                showAgentPanel();
                agentBatchId = data.batch_id;
                pollAgentOutput(data.batch_id, data.agent);
            }
        } else if (resp.status === 409) {
            showAgentPanel();
            const data = await resp.json();
            agentBatchId = data.batch_id;
            pollAgentOutput(data.batch_id, data.agent);
        } else {
            console.warn('[Wormhole] notify-agent failed:', resp.status);
            updateAgentLight('idle');
        }
    } catch (err) {
        console.warn('[Wormhole] notify-agent error:', err.message);
        updateAgentLight('idle');
    }
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
            const resp = await fetch(`${WORMHOLE_BASE}/project/vscode/${projectName}`);
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

            fetch(`${WORMHOLE_BASE}/project/switch/${projectName}?land-in=none`);
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
    const wormholeButtons = document.querySelector('.wormhole-buttons');

    if (vscodeMaximized) {
        container.classList.remove('maximized');
        btn.textContent = 'Maximize';
        document.body.style.overflow = '';
        vscodeMaximized = false;
        if (closeBtn) closeBtn.style.display = '';
        if (wormholeButtons) wormholeButtons.style.display = '';
    } else {
        container.classList.add('maximized');
        btn.textContent = 'Restore';
        document.body.style.overflow = 'hidden';
        vscodeMaximized = true;
        if (closeBtn) closeBtn.style.display = 'none';
        if (wormholeButtons) wormholeButtons.style.display = 'none';
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

function isPrUrl(url) {
    return !!url && /github\.com\/[^/]+\/[^/]+\/pull\/\d+/.test(url);
}

// Resolve the wormhole task name for a button: prefer an already-known name,
// else describe the PR URL, else create the task on demand (gh pr checkout).
async function resolveTaskName(ref, info) {
    if (info?.name) return info.name;
    if (!ref) return null;
    const described = await describeUrl(ref);
    if (described?.name) return described.name;
    return await createFromRef(ref);
}

async function createFromRef(ref) {
    const resp = await fetch(
        `${WORMHOLE_BASE}/project/create-from-github-ref?ref=${encodeURIComponent(ref)}`,
        { method: 'POST' }
    );
    if (!resp.ok) {
        console.warn('[Wormhole] create-from-github-ref failed:', await resp.text());
        return null;
    }
    const result = await resp.json();
    const raw = result.created || result.skipped;
    if (result.error || !raw) {
        console.warn('[Wormhole] create failed:', result.error);
        return null;
    }
    // Result is "repo:branch (created)" or "repo:branch already exists"
    return raw.split(' (')[0].split(' already exists')[0].trim();
}

async function switchByName(name, landIn) {
    const resp = await fetch(`${WORMHOLE_BASE}/project/switch/${name}?land-in=${landIn}`);
    if (!resp.ok) console.warn('[Wormhole] switch failed:', await resp.text());
}

// Open a task in terminal/editor: fast path when it exists, create-then-open otherwise.
async function openTarget(ref, info, landIn, btn) {
    if (btn) { btn.style.opacity = '0.5'; btn.disabled = true; }
    try {
        const name = await resolveTaskName(ref, info);
        if (name) await switchByName(name, landIn);
        else console.warn('[Wormhole] nothing to open (no task or PR ref)');
    } catch (err) {
        console.warn('[Wormhole] openTarget error:', err.message);
    } finally {
        if (btn) { btn.style.opacity = ''; btn.disabled = false; }
    }
}

async function toggleVSCodeFor(ref, info, vscodeBtn) {
    if (vscodeExpanded) {
        toggleVSCode(info?.name, vscodeBtn);
        return;
    }
    const name = await resolveTaskName(ref, info);
    if (name) toggleVSCode(name, vscodeBtn);
    else console.warn('[Wormhole] VSCode: no task to open');
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
            color: #0066cc;
        }
        .wormhole-link-github {
            color: #0066cc;
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
        .wormhole-agent-select {
            font-family: "SF Mono", "Menlo", "Monaco", monospace;
            font-size: 0.65rem;
            padding: 1px 2px;
            border: 1px solid #ccc;
            border-radius: 3px;
            background: #fff;
            color: #555;
            cursor: pointer;
        }
        .wormhole-agent-status {
            display: inline-block;
            width: 8px;
            height: 8px;
            border-radius: 50%;
            transition: background 0.3s;
        }
        .wormhole-agent-idle {
            background: #3fb950;
        }
        .wormhole-agent-running {
            background: #d29922;
            animation: wormhole-pulse 1.5s ease-in-out infinite;
        }
        .wormhole-agent-failed {
            background: #f85149;
        }
        @keyframes wormhole-pulse {
            0%, 100% { opacity: 1; }
            50% { opacity: 0.4; }
        }
        .wormhole-agent-panel {
            display: none;
            position: fixed;
            right: 0;
            top: 0;
            width: 480px;
            height: 100vh;
            background: #1a1a2e;
            color: #e0e0e0;
            font-family: "SF Mono", "Menlo", "Monaco", monospace;
            font-size: 0.8rem;
            z-index: 10000;
            box-shadow: -4px 0 20px rgba(0,0,0,0.4);
            flex-direction: column;
        }
        .wormhole-agent-panel.visible {
            display: flex;
        }
        .wormhole-agent-panel-header {
            display: flex;
            justify-content: space-between;
            align-items: center;
            padding: 0.5rem 0.75rem;
            background: #16213e;
            border-bottom: 1px solid #0f3460;
            flex-shrink: 0;
        }
        .wormhole-agent-panel-title {
            font-weight: 600;
            font-size: 0.75rem;
            color: #a0a0c0;
            text-transform: uppercase;
            letter-spacing: 0.05em;
        }
        .wormhole-agent-panel-close {
            background: none;
            border: none;
            color: #a0a0c0;
            font-size: 1.2rem;
            cursor: pointer;
            padding: 0 0.25rem;
            line-height: 1;
        }
        .wormhole-agent-panel-close:hover {
            color: #fff;
        }
        .wormhole-agent-panel-body {
            flex: 1;
            overflow-y: auto;
            padding: 0.75rem;
            white-space: pre-wrap;
            word-break: break-word;
            line-height: 1.5;
        }
        .wormhole-agent-panel-footer {
            padding: 0.4rem 0.75rem;
            background: #16213e;
            border-top: 1px solid #0f3460;
            font-size: 0.7rem;
            color: #707090;
            flex-shrink: 0;
        }
        .wormhole-stream-meta {
            color: #707090;
        }
        .wormhole-stream-tool-name {
            color: #e94560;
            font-weight: 600;
        }
        .wormhole-stream-tool-input {
            color: #0f3460;
            background: #0d1b2a;
            padding: 0.1rem 0.3rem;
            border-radius: 2px;
        }
    `;
    document.head.appendChild(style);
}

function getTargetSelectors() {
    if (isGitHubPage()) {
        return [
            '.markdown-title',
            'h1[data-component="PH_Title"]',
            '[class*="pr-sticky-title"]',
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
        if (path.match(/^\/(settings|notifications|new|login|signup|pulls)/)) return false;
        return true;
    } else if (isJiraPage()) {
        // /browse/ACT-108 or board view with ?selectedIssue=ACT-108
        return window.location.pathname.includes('/browse/') ||
            window.location.search.includes('selectedIssue=');
    }
    return false;
}

// -- GitHub PR inbox (/pulls) --
// Each PR row gets the same button bar as a PR page (createButtons), keyed on
// the row's PR URL so the task is opened, or created on demand, on click.

function isInboxPage() {
    return isGitHubPage() && window.location.pathname.startsWith('/pulls');
}

const describeCache = new Map();

async function describeUrl(url) {
    if (describeCache.has(url)) return describeCache.get(url);
    try {
        const resp = await fetch(`${WORMHOLE_BASE}/project/describe`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ url })
        });
        if (resp.ok) {
            const info = await resp.json();
            describeCache.set(url, info);
            return info;
        }
    } catch (err) {
        console.warn('[Wormhole] describe error:', err.message);
    }
    return null;
}

function injectInboxButtons() {
    injectStyles();
    const links = document.querySelectorAll('a[data-hovercard-type="pull_request"][href*="/pull/"]');
    for (const link of links) {
        const prUrl = link.href;
        if (!isPrUrl(prUrl)) continue;
        const container = link.closest('[data-listview-item-title-container]')
            || link.closest('h3')?.parentElement;
        if (!container || container.querySelector('.wormhole-buttons')) continue;
        const badges = container.querySelector('[class*="trailingBadgesContainer"]');
        const bar = createButtons(null, prUrl);
        if (!bar) continue;
        (badges || container).appendChild(bar);
        // Lazily upgrade to the task-aware bar (fast switch, agent button) on hover.
        bar.addEventListener('mouseenter', () => enrichInboxRow(prUrl, bar), { once: true });
    }
}

async function enrichInboxRow(prUrl, bar) {
    const info = await describeUrl(prUrl);
    if (info?.name) {
        const fresh = createButtons(info, prUrl);
        if (fresh) bar.replaceWith(fresh);
    }
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
        // These selectors match inline elements; append to parent instead
        if (targetElement && (targetElement.tagName === 'SPAN' || targetElement.tagName === 'BDI')) {
            targetElement = targetElement.parentElement;
        }

        if (targetElement) {
            // Double-check no buttons were added while we waited
            if (document.querySelector('.wormhole-buttons')) return;

            const info = await getDescribe();

            // Triple-check after async call
            if (document.querySelector('.wormhole-buttons')) return;

            const ref = isPrUrl(window.location.href) ? window.location.href : null;
            const buttons = createButtons(info, ref);
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

function dispatch() {
    if (isInboxPage()) injectInboxButtons();
    else injectButtons();
}

// Run on page load
dispatch();

// Re-run on navigation (SPA routing) - debounced
let lastUrl = window.location.href;
let debounceTimer = null;
let ensureTimer = null;

// Guarantees a trailing call even while the page mutates continuously (e.g. the
// PR "changes" diff view), unlike a debounce that a mutation storm keeps resetting.
function scheduleEnsure(fn) {
    if (ensureTimer) return;
    ensureTimer = setTimeout(() => { ensureTimer = null; fn(); }, 200);
}

const observer = new MutationObserver(() => {
    if (window.location.href !== lastUrl) {
        lastUrl = window.location.href;
        retryCount = 0;
        cachedDescribe = null;
        cachedUrl = null;
        vscodeExpanded = false;
        vscodeMaximized = false;
        if (agentPollController) agentPollController.abort();
        agentPollController = null;
        agentBatchId = null;
        closeAgentPanel();
        document.querySelectorAll('.wormhole-buttons').forEach(el => el.remove());
        document.querySelectorAll('.wormhole-vscode-container').forEach(el => el.remove());
        document.body.style.overflow = '';
        clearTimeout(debounceTimer);
        debounceTimer = setTimeout(dispatch, 100);
    } else if (isInboxPage()) {
        scheduleEnsure(injectInboxButtons);
    } else if (!document.querySelector('.wormhole-buttons') && shouldInject() && !injecting) {
        scheduleEnsure(injectButtons);
    }
});
observer.observe(document.body, { childList: true, subtree: true });

// Keyboard shortcut: Ctrl+Shift+N to notify agent
document.addEventListener('keydown', async (e) => {
    if (e.ctrlKey && e.shiftKey && e.key === 'N') {
        const info = cachedDescribe;
        if (info?.task_type === 'review' && info?.name) {
            e.preventDefault();
            notifyAgent(info);
        }
    }
});
