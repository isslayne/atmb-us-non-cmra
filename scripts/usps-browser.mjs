// Use USPS's normal web form so its own scripts initialize the session and request.
// Never import browser profiles, saved cookies, or private user browser data.
import { chromium } from 'playwright';
import { createInterface } from 'node:readline';

const PAGE = 'https://tools.usps.com/zip-code-lookup.htm?byaddress';
const ENDPOINT = 'https://tools.usps.com/tools/app/ziplookup/zipByAddress';
let browser, context, page;
async function initialize() {
  if (browser) return;
  browser = await chromium.launch({ headless: process.env.USPS_HEADLESS !== 'false', ...(process.env.USPS_CHANNEL ? { channel: process.env.USPS_CHANNEL } : {}), ...(process.env.USPS_PROXY ? { proxy: { server: process.env.USPS_PROXY } } : {}) });
  context = await browser.newContext({ locale: 'en-US' });
  page = await context.newPage();
  page.setDefaultTimeout(20000);
}
async function lookup(address) {
  try {
    await initialize();
    await page.goto(PAGE, { waitUntil: 'load', timeout: 45000 });
    await page.waitForLoadState('networkidle', { timeout: 15000 }).catch(() => {});
    if (process.argv.includes('--smoke')) console.error('Scripts:', await page.locator('script[src]').evaluateAll(nodes => nodes.map(n=>n.src).filter(u=>u.startsWith('https://tools.usps.com/'))));
    await page.locator('#tAddress').fill(address.line1);
    await page.locator('#tApt').fill(address.line2 || '');
    await page.locator('#tCity').fill(address.city);
    await page.locator('#tState').selectOption(address.state);
    await page.locator('#tZip-byaddress').fill(address.zip);
    const responsePromise = page.waitForResponse(r => r.url() === ENDPOINT && r.request().method() === 'POST', { timeout: 30000 });
    await page.locator('#zip-by-address').click();
    const response = await responsePromise;
    if (process.argv.includes('--smoke')) console.error('Request header names:', Object.keys(await response.request().allHeaders()).join(', '));
    if (response.status() !== 200) return { error: `http_${response.status()}`, http_status: response.status(), redirect: response.headers().location || '' };
    const result = await response.json();
    if (typeof result.resultStatus !== 'string') return { error: 'invalid_response' };
    return { result };
  } catch (error) {
    return { error: error.name === 'TimeoutError' ? 'browser_timeout' : 'browser_error' };
  }
}
if (process.argv.includes('--smoke')) {
  try {
    const response = await lookup({ line1: '5953 Mabel Rd', line2: 'unit-236', city: 'Las Vegas', state: 'NV', zip: '89110' });
    console.log(JSON.stringify(response));
    if (response.result?.resultStatus !== 'SUCCESS' || !response.result.addressList?.length) process.exitCode = 1;
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
