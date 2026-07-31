import { test, expect } from './fixtures';

// New (React) PR experience header, as served at /pull/N and /pull/N/changes.
// The classic `.markdown-title` class is present, but we also anchor on the
// stable h1[data-component="PH_Title"].
const HEADER = `
<div class="prc-PageHeader-TitleArea-2n2J0" data-component="TitleArea">
  <div class="PullRequestHeader-module__titleWithAction__ODY5f">
    <h1 class="prc-PageHeader-Title-p0Mgh PullRequestHeader-module__inlineTitle__czbud"
        data-component="PH_Title">
      <span class="f1 text-normal markdown-title prc-Text-Text-9mHv3" data-component="Text">Add idempotency for standalone activity operator requests</span>
      <span class="sr-only"> - #11350</span>
    </h1>
  </div>
</div>`;

const PAGE = `<!doctype html><html><head><title>PR</title></head><body>${HEADER}</body></html>`;

const TASK = JSON.stringify({
  name: 'temporal:fredtzeng/saa-req-id-op',
  kind: 'task',
  home_project: 'temporal',
  pr_branch: 'fredtzeng/saa-req-id-op',
});

test.describe('new PR UI', () => {
  for (const path of ['/pull/11350', '/pull/11350/changes']) {
    test(`buttons inject on temporalio/temporal${path}`, async ({ context }) => {
      const page = await context.newPage();
      await page.route(`https://github.com/temporalio/temporal${path}`, (r) =>
        r.fulfill({ contentType: 'text/html', body: PAGE })
      );
      await page.route('http://localhost:7117/**', (r) => {
        const url = new URL(r.request().url());
        if (url.pathname === '/project/describe') {
          return r.fulfill({ contentType: 'application/json', body: TASK });
        }
        return r.fulfill({ contentType: 'application/json', body: '{}' });
      });

      await page.goto(`https://github.com/temporalio/temporal${path}`);

      const buttons = page.locator('.wormhole-buttons');
      await expect(buttons).toBeVisible({ timeout: 10000 });
      await expect(buttons.locator('.wormhole-btn-terminal')).toBeVisible();
      await expect(buttons.locator('.wormhole-btn-cursor')).toBeVisible();
      await expect(buttons.locator('.wormhole-btn-vscode')).toBeVisible();
    });
  }

  test('buttons still appear when the PR has no task yet (create-on-demand)', async ({ context }) => {
    const page = await context.newPage();
    await page.route('https://github.com/temporalio/temporal/pull/11191', (r) =>
      r.fulfill({ contentType: 'text/html', body: PAGE })
    );
    await page.route('http://localhost:7117/**', (r) =>
      r.fulfill({ contentType: 'application/json', body: '{}' })
    );

    await page.goto('https://github.com/temporalio/temporal/pull/11191');

    const buttons = page.locator('.wormhole-buttons');
    await expect(buttons).toBeVisible({ timeout: 10000 });
    await expect(buttons.locator('.wormhole-btn-terminal')).toBeVisible();
  });
});
