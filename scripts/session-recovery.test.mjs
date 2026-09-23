import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withSessionRecovery } from './session-recovery.mjs';

test('302 retries the same address in a new session and returns actual JSON', async () => {
  const address = { line1: '5953 Mabel Rd', line2: 'unit-236', state: 'NV' };
  const result = { result: { resultStatus: 'SUCCESS', addressList: [{ cmar: 'Y' }] } };
  const calls = [];
  const actual = await withSessionRecovery(address, {
    lookup: async a => { assert.equal(a, address); calls.push('lookup'); return calls.length === 1 ? { error: 'http_302' } : result; },
    reset: async () => { calls.push('reset'); },
    pause: async () => { calls.push('pause'); },
  });
  assert.deepEqual(calls, ['lookup', 'reset', 'pause', 'lookup']);
  assert.equal(actual, result);
});

test('a repeated 302 remains an error; it cannot enter an endless retry loop', async () => {
  let calls = 0, resets = 0;
  const actual = await withSessionRecovery({}, {
    lookup: async () => { calls++; return { error: 'http_302' }; },
    reset: async () => { resets++; }, pause: async () => {},
  });
  assert.deepEqual(actual, { error: 'http_302' });
  assert.equal(calls, 2);
  assert.equal(resets, 1);
});

test('successful responses and other errors do not discard the session', async () => {
  for (const result of [{ result: { resultStatus: 'SUCCESS', addressList: [] } }, { error: 'http_429' }]) {
    assert.equal(await withSessionRecovery({}, {
      lookup: async () => result,
      reset: async () => assert.fail('unexpected reset'),
      pause: async () => assert.fail('unexpected pause'),
    }), result);
  }
});
