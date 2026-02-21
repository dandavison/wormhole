const WORKTREE_MARKER = '/wormhole/worktrees/';

export function projectKeyFromPath(fsPath: string): string {
  const idx = fsPath.indexOf(WORKTREE_MARKER);
  if (idx !== -1) {
    const rest = fsPath.substring(idx + WORKTREE_MARKER.length);
    const parts = rest.split('/').filter((p) => p);
    if (parts.length >= 2) {
      const repo = parts[parts.length - 1];
      const encodedBranch = parts.slice(0, -1).join('/');
      const branch = encodedBranch.replace(/--/g, '/');
      return `${repo}:${branch}`;
    }
  }
  const leaf = fsPath
    .split('/')
    .filter((p) => p)
    .pop();
  return leaf || '';
}
