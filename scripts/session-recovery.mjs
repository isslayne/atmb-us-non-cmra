// Do not turn a web redirect into a successful validation or retry indefinitely.
export async function withSessionRecovery(address, { lookup, reset, pause }) {
  const result = await lookup(address);
  if (result.error !== 'http_302') return result;
  await reset();
  await pause();
  return lookup(address);
}
