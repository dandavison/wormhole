import * as vscode from 'vscode';
import * as http from 'http';
import { projectKeyFromPath } from './project-key';

type IntentHandler = (
  projectKey: string,
  port: number,
) => void | Thenable<void>;

function vscodeCommand(command: string): IntentHandler {
  return () => vscode.commands.executeCommand(command);
}

const INTENTS: Record<string, IntentHandler> = {
  'editor/close': vscodeCommand('workbench.action.closeWindow'),
  'editor/toggleZenMode': vscodeCommand('workbench.action.toggleZenMode'),
  echo: (projectKey, port) => putKv(projectKey, port, 'last-message', 'echo'),
};

let abortController: AbortController | null = null;
let statusItem: vscode.StatusBarItem | null = null;

export function activate(context: vscode.ExtensionContext) {
  const projectKey = resolveProjectKey();
  if (!projectKey) {
    return;
  }
  const config = vscode.workspace.getConfiguration('wormhole');
  const port =
    config.get<number>('port') ??
    parseInt(process.env.WORMHOLE_PORT || '7117', 10);

  statusItem = vscode.window.createStatusBarItem(
    vscode.StatusBarAlignment.Left,
    0,
  );
  const version = context.extension.packageJSON.version;
  statusItem.text = '🌀';
  statusItem.tooltip = `wormhole ${version}: ${projectKey} (port ${port})`;
  statusItem.show();
  context.subscriptions.push(statusItem);

  abortController = new AbortController();
  pollLoop(projectKey, port, abortController.signal);
}

export function deactivate() {
  abortController?.abort();
  abortController = null;
}

async function pollLoop(projectKey: string, port: number, signal: AbortSignal) {
  while (!signal.aborted) {
    try {
      const messages = await poll(projectKey, port, signal);
      for (const msg of messages) {
        const handler = INTENTS[msg.method];
        if (handler) {
          await handler(projectKey, port);
        }
      }
    } catch (e: unknown) {
      if (signal.aborted) {
        return;
      }
      await sleep(2000);
    }
  }
}

interface Notification {
  jsonrpc: string;
  method: string;
  params?: Record<string, unknown>;
}

function poll(
  projectKey: string,
  port: number,
  signal: AbortSignal,
): Promise<Notification[]> {
  return new Promise((resolve, reject) => {
    const req = http.get(
      `http://127.0.0.1:${port}/project/messages/${projectKey}?role=editor&wait=30`,
      (res) => {
        let data = '';
        res.on('data', (chunk: string) => (data += chunk));
        res.on('end', () => {
          try {
            resolve(JSON.parse(data) as Notification[]);
          } catch {
            resolve([]);
          }
        });
      },
    );
    req.on('error', reject);
    signal.addEventListener(
      'abort',
      () => {
        req.destroy();
        reject(new Error('aborted'));
      },
      { once: true },
    );
  });
}

function resolveProjectKey(): string | undefined {
  const folders = vscode.workspace.workspaceFolders;
  if (!folders || folders.length === 0) {
    return undefined;
  }
  return projectKeyFromPath(folders[0].uri.fsPath);
}

function putKv(projectKey: string, port: number, key: string, value: string) {
  const req = http.request(`http://127.0.0.1:${port}/kv/${projectKey}/${key}`, {
    method: 'PUT',
  });
  req.on('error', () => {});
  req.end(value);
}

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
