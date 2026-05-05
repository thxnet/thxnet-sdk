import { ApiPromise, HttpProvider } from '@polkadot/api';
const main = async () => {
  for (const [label, port] of [['alice', 9931], ['bob', 9932], ['charlie', 9933]]) {
    try {
      const api = await ApiPromise.create({ provider: new HttpProvider(`http://localhost:${port}`) });
      await api.isReady;
      const peers = (await (api.rpc as any).system.peers()).toJSON();
      console.log(`${label} peers (${peers.length}):`);
      for (const p of peers) console.log(`  ${p.peerId} role=${p.roles}`);
      await api.disconnect();
    } catch (e: any) { console.log(`${label}: ${e.message}`); }
  }
  process.exit(0);
};
main().catch(e => { console.error(e); process.exit(1); });
