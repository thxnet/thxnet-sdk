import { ApiPromise, HttpProvider } from '@polkadot/api';
const main = async () => {
  const api = await ApiPromise.create({ provider: new HttpProvider('http://localhost:9931') });
  await api.isReady;
  const keys = await api.query.session.queuedKeys();
  for (const [validator, sk] of keys.toJSON() as any[]) {
    console.log('validator:', validator, 'authorityDiscovery:', sk.authorityDiscovery);
  }
  const queuedChanged = await api.query.session.queuedChanged();
  console.log('queuedChanged:', queuedChanged.toJSON());
  const ci = await api.query.session.currentIndex();
  console.log('currentIndex:', ci.toJSON());
  process.exit(0);
};
main().catch(e => { console.error(e); process.exit(1); });
