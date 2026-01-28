import { test as base, chromium, type BrowserContext } from '@playwright/test';
import path from 'path';

export type TestFixtures = {
  context: BrowserContext;
};

const headed = process.env.HEADED === '1';

export const test = base.extend<TestFixtures>({
  context: async ({}, use) => {
    const extensionPath = path.resolve(__dirname, '..');
    const args = [
      `--disable-extensions-except=${extensionPath}`,
      `--load-extension=${extensionPath}`,
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-features=DialMediaRouteProvider,WebAuthn',
      '--disable-component-update',
      '--disable-background-networking',
      '--disable-sync',
      '--disable-translate',
      '--disable-device-discovery-notifications',
      '--disable-web-security',
      '--allow-running-insecure-content',
    ];
    if (!headed) {
      args.push('--headless=new');
    }
    const context = await chromium.launchPersistentContext('', {
      headless: false,  // Required for extensions, but --headless=new overrides
      args,
    });
    await use(context);
    await context.close();
  },
});

export { expect } from '@playwright/test';
