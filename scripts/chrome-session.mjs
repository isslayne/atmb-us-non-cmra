import { chromium } from 'playwright';
import { Server } from 'proxy-chain';
import { spawn } from 'node:child_process';
import { existsSync } from 'node:fs';
import { mkdtemp, rm } from 'node:fs/promises';
import { createServer } from 'node:net';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

function executable() {
  const candidates = [
    process.env.USPS_BROWSER_EXECUTABLE,
    ...(process.platform === 'win32' ? [
      join(process.env.PROGRAMFILES || 'C:\\Program Files', 'Google/Chrome/Application/chrome.exe'),
      join(process.env['PROGRAMFILES(X86)'] || 'C:\\Program Files (x86)', 'Google/Chrome/Application/chrome.exe'),
      join(process.env.LOCALAPPDATA || '', 'Google/Chrome/Application/chrome.exe'),
    ] : process.platform === 'darwin' ? [
      '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    ] : ['/opt/google/chrome/chrome', '/usr/bin/google-chrome', '/usr/bin/chromium']),
  ];
  const path = candidates.find(p => p && existsSync(p));
  if (!path) throw new Error('Chrome not installed; run npx playwright install chrome');
  return path;
}

async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', resolve);
  });
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}

// A fresh, normal Chrome profile. Never connect to the user's running browser.
// USPS treats Playwright's isolated browser contexts differently from normal tabs.
export async function chromeSession() {
  const profile = await mkdtemp(join(tmpdir(), 'atmb-usps-'));
  let browser, child, proxy, closed = false;
  const close = async () => {
    if (closed) return;
    closed = true;
    if (browser) {
      try {
        const session = await browser.newBrowserCDPSession();
        await session.send('Browser.close');
      } catch { /* Chrome can close before acknowledging Browser.close. */ }
      await browser.close().catch(() => {});
    }
    if (child && child.exitCode === null) {
      const exited = new Promise(resolve => child.once('exit', resolve));
      await Promise.race([exited, delay(3000)]);
      if (child.exitCode === null) {
        child.kill();
        await Promise.race([exited, delay(3000)]);
      }
    }
    await proxy?.close(true);
    await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  };
  try {
    const port = await freePort();
    const args = [
      `--remote-debugging-port=${port}`, '--remote-debugging-address=127.0.0.1',
      `--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check',
    ];
    if (process.env.USPS_HEADLESS === 'true') args.push('--headless=new');
    if (process.env.USPS_PROXY) {
      const upstream = new URL(process.env.USPS_PROXY);
      if (!['http:', 'https:'].includes(upstream.protocol)) throw new Error('Unsupported USPS proxy protocol');
      // Keep proxy credentials out of Chrome's command line and log output.
      proxy = new Server({
        host: '127.0.0.1', port: 0,
        prepareRequestFunction: () => ({ upstreamProxyUrl: upstream.href }),
      });
      await proxy.listen();
      args.push(`--proxy-server=http://127.0.0.1:${proxy.port}`);
    }
    args.push('about:blank');
    let spawnFailed = false;
    const env = { ...process.env };
    for (const key of ['CREDENTIALS', 'SMARTY_AUTH_ID', 'SMARTY_AUTH_TOKEN', 'USPS_PROXY']) delete env[key];
    child = spawn(executable(), args, { stdio: 'ignore', env });
    child.once('error', () => { spawnFailed = true; });
    const endpoint = `http://127.0.0.1:${port}`;
    for (let attempt = 0; attempt < 100; attempt++) {
      if (spawnFailed || child.exitCode !== null) throw new Error('Chrome failed to start');
      try {
        const response = await fetch(`${endpoint}/json/version`, { signal: AbortSignal.timeout(500) });
        if (response.ok) {
          browser = await chromium.connectOverCDP(endpoint, { timeout: 5000 });
          break;
        }
      } catch { /* The debugging endpoint is not ready yet. */ }
      await delay(200);
    }
    if (!browser) throw new Error('Chrome startup timed out');
    const context = browser.contexts()[0];
    const page = await context.newPage();
    return { page, close };
  } catch (error) {
    await close();
    throw error;
  }
}
