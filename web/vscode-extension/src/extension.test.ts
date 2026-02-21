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

test('task worktree', () => {
  assert.strictEqual(
    projectKeyFromPath(
      '/Users/dan/src/wormhole/.git/wormhole/worktrees/messages/wormhole',
    ),
    'wormhole:messages',
  );
});

test('task with slash in branch', () => {
  assert.strictEqual(
    projectKeyFromPath('/repo/.git/wormhole/worktrees/feature--auth/myrepo'),
    'myrepo:feature/auth',
  );
});

test('task with nested slash in branch', () => {
  assert.strictEqual(
    projectKeyFromPath(
      '/repo/.git/wormhole/worktrees/user--nested--deep/myrepo',
    ),
    'myrepo:user/nested/deep',
  );
});

test('non-task project', () => {
  assert.strictEqual(projectKeyFromPath('/Users/dan/src/wormhole'), 'wormhole');
});

test('non-task project trailing slash', () => {
  assert.strictEqual(projectKeyFromPath('/Users/dan/src/delta/'), 'delta');
});
