import { projectKeyFromPath } from './project-key';
import * as assert from 'assert';

function test(name: string, fn: () => void) {
  try {
    fn();
    console.log(`  ok: ${name}`);
  } catch (e: unknown) {
    console.error(`  FAIL: ${name}`);
    console.error(`    ${e}`);
    process.exitCode = 1;
  }
}

console.log('projectKeyFromPath');

const worktreeDir = '/Users/dan/worktrees';

test('task worktree', () => {
  assert.strictEqual(
    projectKeyFromPath(
      '/Users/dan/worktrees/wormhole/messages/wormhole',
      worktreeDir,
    ),
    'wormhole:messages',
  );
});

test('task with slash in branch', () => {
  assert.strictEqual(
    projectKeyFromPath(
      '/Users/dan/worktrees/myrepo/feature--auth/myrepo',
      worktreeDir,
    ),
    'myrepo:feature/auth',
  );
});

test('task with nested slash in branch', () => {
  assert.strictEqual(
    projectKeyFromPath(
      '/Users/dan/worktrees/myrepo/user--nested--deep/myrepo',
      worktreeDir,
    ),
    'myrepo:user/nested/deep',
  );
});

test('non-task project', () => {
  assert.strictEqual(
    projectKeyFromPath('/Users/dan/src/wormhole', worktreeDir),
    'wormhole',
  );
});

test('non-task project trailing slash', () => {
  assert.strictEqual(
    projectKeyFromPath('/Users/dan/src/delta/', worktreeDir),
    'delta',
  );
});

test('no worktreeDir falls back to leaf', () => {
  assert.strictEqual(
    projectKeyFromPath('/Users/dan/worktrees/myrepo/branch/myrepo'),
    'myrepo',
  );
});
