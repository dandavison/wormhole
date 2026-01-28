import { test, expect } from './fixtures';

test.describe('GitHub Integration', () => {
  test('PR page shows wormhole buttons', async ({ context }) => {
    const page = await context.newPage();
    await page.goto('https://github.com/temporalio/temporal/pull/9099');

    const buttons = page.locator('.wormhole-buttons');
    await expect(buttons).toBeVisible({ timeout: 10000 });

    await expect(buttons.locator('.wormhole-btn-terminal')).toBeVisible();
    await expect(buttons.locator('.wormhole-btn-cursor')).toBeVisible();
    await expect(buttons.locator('.wormhole-btn-vscode')).toBeVisible();
  });

  test('repo page shows wormhole buttons', async ({ context }) => {
    const page = await context.newPage();
    await page.goto('https://github.com/temporalio/temporal');

    const buttons = page.locator('.wormhole-buttons');
    await expect(buttons).toBeVisible({ timeout: 10000 });

    await expect(buttons.locator('.wormhole-btn-terminal')).toBeVisible();
    await expect(buttons.locator('.wormhole-btn-cursor')).toBeVisible();
  });

  test('VSCode button opens embedded editor', async ({ context }) => {
    const page = await context.newPage();
    await page.goto('https://github.com/temporalio/temporal/pull/9099');

    const buttons = page.locator('.wormhole-buttons');
    await expect(buttons).toBeVisible({ timeout: 10000 });

    const vscodeBtn = buttons.locator('.wormhole-btn-vscode');
    await vscodeBtn.click();

    const container = page.locator('.wormhole-vscode-container.expanded');
    await expect(container).toBeVisible({ timeout: 15000 });

    await expect(container.locator('.wormhole-toolbar-maximize')).toBeVisible();
    await expect(container.locator('.wormhole-toolbar-close')).toBeVisible();
    await expect(container.locator('iframe')).toBeVisible();
  });

  test('maximize and restore VSCode', async ({ context }) => {
    const page = await context.newPage();
    await page.goto('https://github.com/temporalio/temporal/pull/9099');

    const buttons = page.locator('.wormhole-buttons');
    await expect(buttons).toBeVisible({ timeout: 10000 });

    await buttons.locator('.wormhole-btn-vscode').click();

    const container = page.locator('.wormhole-vscode-container.expanded');
    await expect(container).toBeVisible({ timeout: 15000 });

    const maximizeBtn = container.locator('.wormhole-toolbar-maximize');
    await maximizeBtn.click();

    await expect(container).toHaveClass(/maximized/);
    await expect(maximizeBtn).toHaveText('Restore');

    await maximizeBtn.click();

    await expect(container).not.toHaveClass(/maximized/);
    await expect(maximizeBtn).toHaveText('Maximize');
  });

  test('close VSCode via toolbar', async ({ context }) => {
    const page = await context.newPage();
    await page.goto('https://github.com/temporalio/temporal/pull/9099');

    const buttons = page.locator('.wormhole-buttons');
    await expect(buttons).toBeVisible({ timeout: 10000 });

    await buttons.locator('.wormhole-btn-vscode').click();

    const container = page.locator('.wormhole-vscode-container');
    await expect(container).toHaveClass(/expanded/, { timeout: 15000 });

    await container.locator('.wormhole-toolbar-close').click();

    await expect(container).not.toHaveClass(/expanded/);
  });
});

// JIRA tests skipped - require authentication
// TODO: Add JIRA tests once we have a way to handle auth (e.g., storageState)
test.describe.skip('JIRA Integration', () => {
  test('issue page shows wormhole buttons', async ({ context }) => {
    const page = await context.newPage();
    await page.goto('https://temporalio.atlassian.net/browse/ACT-108');

    const buttons = page.locator('.wormhole-buttons');
    await expect(buttons).toBeVisible({ timeout: 10000 });

    await expect(buttons.locator('.wormhole-btn-terminal')).toBeVisible();
    await expect(buttons.locator('.wormhole-btn-cursor')).toBeVisible();
    await expect(buttons.locator('.wormhole-btn-vscode')).toBeVisible();
  });

  test('issue page shows GitHub link', async ({ context }) => {
    const page = await context.newPage();
    await page.goto('https://temporalio.atlassian.net/browse/ACT-108');

    const buttons = page.locator('.wormhole-buttons');
    await expect(buttons).toBeVisible({ timeout: 10000 });

    const githubLink = buttons.locator('.wormhole-link-github');
    await expect(githubLink).toBeVisible();
    await expect(githubLink).toHaveAttribute('href', /github\.com/);
  });
});
