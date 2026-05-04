import { ApiPromise, WsProvider } from '@polkadot/api';
async function main() {
  const api = await ApiPromise.create({ provider: new WsProvider('ws://localhost:9934') });
  await api.isReady;
  const head = await api.rpc.chain.getHeader();
  const blockNum = head.number.toNumber();
  console.log(`para best block: #${blockNum}`);
  const ver = api.runtimeVersion;
  console.log(`spec=${ver.specName.toString()} v${ver.specVersion.toNumber()}`);
  // Check the codeUpdated event was emitted within last 50 blocks
  let codeUpdatedFound = false;
  let codeUpdatedAt = -1;
  for (let i = blockNum; i >= Math.max(0, blockNum - 50); i--) {
    try {
      const hash = await api.rpc.chain.getBlockHash(i);
      const apiAt = await api.at(hash);
      const events = await apiAt.query.system.events();
      for (const ev of events.toArray()) {
        const e = ev.event;
        if (e.section === 'parachainSystem' && (e.method === 'ValidationFunctionApplied' || e.method === 'ValidationFunctionStored' || e.method === 'UpwardMessageSent')) {
          console.log(`block #${i} event: ${e.section}.${e.method} ${e.data.toString()}`);
          if (e.method === 'ValidationFunctionStored') codeUpdatedAt = i;
        }
        if (e.section === 'system' && e.method === 'CodeUpdated') {
          codeUpdatedFound = true;
          codeUpdatedAt = i;
          console.log(`block #${i} system.CodeUpdated`);
        }
      }
    } catch (e) {}
  }
  console.log(`CodeUpdated within 50-block window: ${codeUpdatedFound} (at block #${codeUpdatedAt})`);
  // Also check parachain is still advancing
  await new Promise(r => setTimeout(r, 8000));
  const head2 = await api.rpc.chain.getHeader();
  console.log(`para best block 8s later: #${head2.number.toNumber()} (delta=${head2.number.toNumber() - blockNum})`);
  process.exit(0);
}
main().catch(e => { console.error(e); process.exit(1); });
