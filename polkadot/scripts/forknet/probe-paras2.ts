import { ApiPromise, HttpProvider } from '@polkadot/api';
const main = async () => {
  const api = await ApiPromise.create({ provider: new HttpProvider('http://localhost:9931') });
  await api.isReady;
  const session = await api.query.session.validators();
  console.log('session.validators count:', (session as any).length);
  console.log('session.validators:', (session.toHuman() as any[]).slice(0, 4));
  const queuedKeys = await api.query.session.queuedKeys();
  console.log('queuedKeys count:', (queuedKeys as any).length);
  const sess0 = await api.query.paraSessionInfo.sessions(0);
  if ((sess0 as any).isSome) {
    const v = (sess0 as any).unwrap();
    console.log('paraSessionInfo.sessions(0).validators:', v.validators.length);
    console.log('  active_validator_indices:', JSON.stringify(v.activeValidatorIndices.toJSON()));
    console.log('  validator_groups:', JSON.stringify(v.validatorGroups.toJSON()));
    console.log('  n_cores:', v.nCores.toNumber());
    console.log('  needed_approvals:', v.neededApprovals.toNumber());
    console.log('  discovery_keys count:', v.discoveryKeys.length);
    console.log('  assignment_keys count:', v.assignmentKeys.length);
  } else {
    console.log('paraSessionInfo.sessions(0) is None');
  }
  const cs = await api.query.paras.currentCodeHash(1003);
  console.log('paras.currentCodeHash(1003):', (cs as any).toHex());
  const finalSess = await api.query.parasShared.currentSessionIndex();
  console.log('parasShared.currentSessionIndex:', (finalSess as any).toNumber());
  const av = await api.query.parasShared.activeValidatorKeys();
  console.log('parasShared.activeValidatorKeys count:', (av as any).length);
  process.exit(0);
};
main().catch(e => { console.error(e); process.exit(1); });
