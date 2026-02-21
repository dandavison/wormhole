import * as vscode from 'vscode';
import * as http from 'http';
import { projectKeyFromPath } from './project-key';

const INTENTS: Record<string, string> = {
  'editor/close': 'workbench.action.closeWindow',
  'editor/toggleZenMode': 'workbench.action.toggleZenMode',
};

let abortController: AbortController | null = null;

export function activate(context: vscode.ExtensionContext) {
  const projectKey = resolveProjectKey();
  if (!projectKey) {
    return;
  }
  const port = parseInt(process.env.WORMHOLE_PORT || '7117', 10);
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
        const command = INTENTS[msg.method];
        if (command) {
          await vscode.commands.executeCommand(command);
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
      `http://127.0.0.1:${port}/project/messages/${encodeURIComponent(projectKey)}?role=editor&wait=30`,
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

function sleep(ms: number): Promise<void> {
  return new Promise((r) => setTimeout(r, ms));
}
