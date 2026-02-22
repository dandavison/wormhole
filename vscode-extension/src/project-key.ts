export function projectKeyFromPath(
  fsPath: string,
  worktreeDir?: string,
): string {
  if (worktreeDir) {
    const prefix = worktreeDir.endsWith('/') ? worktreeDir : worktreeDir + '/';
    if (fsPath.startsWith(prefix)) {
      // Path is $worktreeDir/$repo/$encodedBranch/$repo
      const rest = fsPath.substring(prefix.length);
      const parts = rest.split('/').filter((p) => p);
      if (parts.length >= 3) {
        const repo = parts[0];
        const encodedBranch = parts[1];
        const branch = encodedBranch.replace(/--/g, '/');
        return `${repo}:${branch}`;
      }
    }
  }
  const leaf = fsPath
    .split('/')
    .filter((p) => p)
    .pop();
  return leaf || '';
}
