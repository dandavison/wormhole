import { test, expect } from './fixtures';

// One PR row copied from github.com/pulls/inbox (class hashes are stable enough
// for the selectors we rely on: the pull_request hovercard anchor + the
// trailing-badges container).
const ROW = `
<li class="ListItem-module__listItem__wBJcm">
  <div class="Title-module__container__ZzhV_" data-listview-item-title-container="true">
    <h3 class="Title-module__heading__tHuYV">
      <a class="TitleInlineCode-module__inlineCodeTitle__yr9qP"
         href="https://github.com/temporalio/temporal/pull/11344"
         data-hovercard-url="/temporalio/temporal/pull/11344/hovercard"
         data-hovercard-type="pull_request">
        <span><span>Fix standalone activity mutation retry deduplication</span></span>
      </a>
    </h3>
    <span class="Title-module__trailingBadgesContainer__INeSa"></span>
  </div>
</li>`;

const INBOX_HTML = `<!doctype html><html><head><title>Pull requests</title></head>
<body><ul>${ROW}</ul></body></html>`;

test.describe('GitHub PR inbox', () => {
  test('each PR row gets a jump button that creates+switches the task', async ({ context }) => {
    const page = await context.newPage();

    let createRef: string | null = null;
    let switchTarget: string | null = null;

    await page.route('https://github.com/pulls/inbox', (route) =>
      route.fulfill({ contentType: 'text/html', body: INBOX_HTML })
    );

    await page.route('http://localhost:7117/**', (route) => {
      const url = new URL(route.request().url());
      if (url.pathname === '/project/create-from-github-ref') {
        createRef = url.searchParams.get('ref');
        return route.fulfill({
          contentType: 'application/json',
          body: JSON.stringify({ created: 'temporal:fix-standalone (created)' }),
        });
      }
      if (url.pathname.startsWith('/project/switch/')) {
        switchTarget = url.pathname + '?' + url.searchParams.toString();
        return route.fulfill({ contentType: 'application/json', body: '{}' });
      }
      return route.fulfill({ status: 404, body: '' });
    });

    await page.goto('https://github.com/pulls/inbox');

    const btn = page.locator('.wormhole-inbox-btn');
    await expect(btn).toBeVisible({ timeout: 10000 });

    await btn.click();

    await expect
      .poll(() => createRef, { timeout: 5000 })
      .toBe('https://github.com/temporalio/temporal/pull/11344');
    await expect
      .poll(() => switchTarget, { timeout: 5000 })
      .toContain('/project/switch/temporal:fix-standalone');
    await expect.poll(() => switchTarget).toContain('land-in=terminal-only');
  });
});
