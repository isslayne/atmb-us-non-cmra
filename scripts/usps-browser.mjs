// Use USPS's normal web form so its own scripts initialize the session and request.
// Never import browser profiles, saved cookies, or private user browser data.
import { chromeSession } from './chrome-session.mjs';
import { withSessionRecovery } from './session-recovery.mjs';
import { createInterface } from 'node:readline';
import { setTimeout as delay } from 'node:timers/promises';

const PAGE = 'https://tools.usps.com/zip-code-lookup.htm?byaddress';
const ENDPOINT = 'https://tools.usps.com/tools/app/ziplookup/zipByAddress';
let browser, page, ready = false;
async function initialize() {
  if (browser) return;
  browser = await chromeSession();
  page = browser.page;
  page.setDefaultTimeout(20000);
}
async function lookupOnce(address) {
  try {
    await initialize();
    if (!ready) {
      await page.goto(PAGE, { waitUntil: 'load', timeout: 45000 });
      await page.waitForLoadState('networkidle', { timeout: 15000 }).catch(() => {});
      ready = true;
    } else if (!(await page.locator('#tAddress').isVisible())) {
      await page.locator('#search-address-again').click();
    }
    await page.locator('#tAddress').fill(address.line1);
    await page.locator('#tApt').fill(address.line2 || '');
    await page.locator('#tCity').fill(address.city);
    await page.locator('#tState').selectOption(address.state);
    await page.locator('#tZip-byaddress').fill(address.zip);
    const [response] = await Promise.all([
      page.waitForResponse(r => r.url() === ENDPOINT && r.request().method() === 'POST', { timeout: 30000 }),
      page.locator('#zip-by-address').click(),
    ]);
    if (response.status() !== 200) return { error: `http_${response.status()}`, http_status: response.status(), redirect: response.headers().location || '' };
    const result = await response.json();
    if (typeof result.resultStatus !== 'string') return { error: 'invalid_response' };
    return { result };
  } catch (error) {
    ready = false;
    return { error: error.name === 'TimeoutError' ? 'browser_timeout' : 'browser_error' };
  }
}
async function lookup(address) {
  return withSessionRecovery(address, {
    lookup: lookupOnce,
    reset: async () => {
      await browser?.close();
      browser = undefined;
      ready = false;
    },
    pause: () => delay(3000),
  });
}
for (const signal of ['SIGINT', 'SIGTERM']) {
  process.once(signal, async () => { await browser?.close(); process.exit(0); });
}
if (process.argv.includes('--smoke')) {
  try {
    for (const address of [
      { line1: '14845 SW Murray Scholls Dr', line2: 'Ste 110', city: 'Beaverton', state: 'OR', zip: '97007' },
      { line1: '5953 Mabel Rd', line2: 'unit-236', city: 'Las Vegas', state: 'NV', zip: '89110' },
      { line1: '243 E 5th Ave', line2: '', city: 'Anchorage', state: 'AK', zip: '99501' },
    ]) {
      const response = await lookup(address);
      console.log(JSON.stringify(response));
      if (response.result?.resultStatus !== 'SUCCESS' || !response.result.addressList?.length) process.exitCode = 1;
    }
  } finally { await browser?.close(); }
} else {
  const input = createInterface({ input: process.stdin, crlfDelay: Infinity });
  try {
    for await (const line of input) {
      let response;
      try { response = await lookup(JSON.parse(line)); }
      catch { response = { error: 'invalid_input' }; }
      process.stdout.write(JSON.stringify(response) + '\n');
    }
  } finally { await browser?.close(); }
}
