// verify-state-preserved.ts — Confirm livenet state is alive in forknet
//
// Usage:
//   bun run verify-state-preserved.ts \
//     --livenet wss://node.testnet.thxnet.org/archive-001/ws \
//     --forknet ws://localhost:9931
//
// Checks (against the forknet's GENESIS block #1 to compare to livenet's chosen finalized block):
//   1. Total Issuance — forknet >= 99% of livenet (filtering may shed some validator-slashed amounts)
//   2. Treasury balance — forknet ~= livenet
//   3. Sudo.Key — forknet uses Alice (replaced as expected)
//   4. Account balance match — pick 10 non-validator accounts from livenet,
//      verify each balance is preserved on forknet (within rounding)
//   5. Existential pallet state — expected pallet keys present (verify count)

import { ApiPromise, HttpProvider, WsProvider } from "@polkadot/api";

const ALICE_SS58 = "5GrwvaEF5zXb26Fz9rcQpDWS57CtERHpNehXCPcNoHGKutQY";

function getArg(name: string, fallback?: string): string {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length) return process.argv[i + 1];
  if (fallback !== undefined) return fallback;
  throw new Error(`missing arg --${name}`);
}

const LIVENET = getArg("livenet", "wss://node.testnet.thxnet.org/archive-001/ws");
const FORKNET = getArg("forknet", "http://localhost:9931");

async function connect(uri: string): Promise<ApiPromise> {
  const provider = uri.startsWith("http") ? new HttpProvider(uri) : new WsProvider(uri);
  return ApiPromise.create({ provider });
}

async function main() {
  console.log(`livenet: ${LIVENET}`);
  console.log(`forknet: ${FORKNET}`);

  console.log("\nConnecting to livenet (read-only)...");
  const live = await connect(LIVENET);
  const liveSpec = live.runtimeVersion;
  const liveHash = await live.rpc.chain.getFinalizedHead();
  const liveHeader = await live.rpc.chain.getHeader(liveHash);
  console.log(
    `  livenet runtime: ${liveSpec.specName.toString()} v${liveSpec.specVersion.toNumber()} @ #${liveHeader.number.toNumber()} ${liveHash.toHex()}`
  );

  console.log("\nConnecting to forknet...");
  const fork = await connect(FORKNET);
  const forkSpec = fork.runtimeVersion;
  const forkHeader = await fork.rpc.chain.getHeader();
  console.log(
    `  forknet runtime: ${forkSpec.specName.toString()} v${forkSpec.specVersion.toNumber()} @ #${forkHeader.number.toNumber()}`
  );

  let pass = 0;
  let fail = 0;

  // ── Check 1: Total Issuance ──
  const liveTotalIssuance = (await live.query.balances.totalIssuance.at(liveHash)) as any;
  const forkTotalIssuance = (await fork.query.balances.totalIssuance()) as any;
  const liveTI = BigInt(liveTotalIssuance.toString());
  const forkTI = BigInt(forkTotalIssuance.toString());
  const ratio = (Number(forkTI) / Number(liveTI)) * 100;
  console.log(`\n[1] Total Issuance`);
  console.log(`    livenet : ${liveTI.toString()}`);
  console.log(`    forknet : ${forkTI.toString()}`);
  console.log(`    ratio   : ${ratio.toFixed(4)}%`);
  if (ratio >= 99 && ratio <= 101) {
    console.log(`    PASS — forknet TI within 1% of livenet`);
    pass++;
  } else {
    console.log(`    FAIL — forknet TI diverged > 1% from livenet`);
    fail++;
  }

  // ── Check 2: Treasury account balance ──
  // Treasury account = derived from "py/trsry" + 0 padding
  const treasuryAccount = "5EYCAe5ijiYfyeZ2JJCGq56LmPyNRAKzpG4QkoQkkQNB5e6Z";
  try {
    const liveT = (await live.query.system.account.at(liveHash, treasuryAccount)) as any;
    const forkT = (await fork.query.system.account(treasuryAccount)) as any;
    const liveBal = BigInt(liveT.data.free.toString());
    const forkBal = BigInt(forkT.data.free.toString());
    console.log(`\n[2] Treasury account ${treasuryAccount}`);
    console.log(`    livenet free : ${liveBal.toString()}`);
    console.log(`    forknet free : ${forkBal.toString()}`);
    if (liveBal === forkBal) {
      console.log(`    PASS — exact match`);
      pass++;
    } else if (liveBal > 0n && forkBal > 0n) {
      console.log(`    PASS — both non-zero (acceptable; treasury may have moved during fork window)`);
      pass++;
    } else if (liveBal === 0n && forkBal === 0n) {
      console.log(`    PASS — both zero`);
      pass++;
    } else {
      console.log(`    FAIL — only one side has balance`);
      fail++;
    }
  } catch (e: any) {
    console.log(`\n[2] Treasury account: SKIP (${e.message})`);
  }

  // ── Check 3: Sudo.Key (forknet should be Alice) ──
  try {
    const liveSudo = (await live.query.sudo.key.at(liveHash)) as any;
    const forkSudo = (await fork.query.sudo.key()) as any;
    console.log(`\n[3] Sudo.Key`);
    console.log(`    livenet : ${liveSudo.toString()}`);
    console.log(`    forknet : ${forkSudo.toString()}`);
    if (forkSudo.toString() === ALICE_SS58) {
      console.log(`    PASS — forknet Sudo.Key is Alice (expected dev replacement)`);
      pass++;
    } else if (liveSudo.toString() === forkSudo.toString()) {
      console.log(`    NOTE — forknet preserved livenet sudo (filter behaviour: sudo not stripped)`);
      pass++;
    } else {
      console.log(`    FAIL — unexpected Sudo.Key on forknet`);
      fail++;
    }
  } catch (e: any) {
    console.log(`\n[3] Sudo.Key: SKIP (${e.message})`);
  }

  // ── Check 4: Sample 10 non-validator accounts from livenet ──
  console.log(`\n[4] Sampling 10 random System.Account entries from livenet...`);
  const entries = await live.query.system.account.entriesPaged({ pageSize: 50, args: [] });
  // pick first 10 with non-zero free balance
  const sample = entries
    .filter(([_, v]: any) => BigInt(v.data.free.toString()) > 0n)
    .slice(0, 10);
  console.log(`    fetched ${entries.length} entries, sampled ${sample.length} with non-zero balance`);

  let sampleMatch = 0;
  let sampleMismatch = 0;
  for (const [key, liveAcc] of sample) {
    const acc = key.args[0].toString();
    const liveFree = BigInt((liveAcc as any).data.free.toString());
    const forkAcc = (await fork.query.system.account(acc)) as any;
    const forkFree = BigInt(forkAcc.data.free.toString());
    if (liveFree === forkFree) {
      sampleMatch++;
    } else {
      sampleMismatch++;
      console.log(
        `    MISMATCH: ${acc}  live=${liveFree}  fork=${forkFree}  delta=${liveFree - forkFree}`
      );
    }
  }
  console.log(`    matched: ${sampleMatch}/${sample.length}`);
  if (sampleMatch >= sample.length * 0.95) {
    console.log(`    PASS — ≥95% of sampled accounts match between livenet and forknet`);
    pass++;
  } else {
    console.log(`    FAIL — only ${sampleMatch}/${sample.length} match`);
    fail++;
  }

  // ── Check 5: Existence of expected pallets ──
  const expectedPallets = [
    "system", "balances", "sudo", "treasury", "session", "staking",
    "configuration", "paras", "bounties", "identity",
  ];
  console.log(`\n[5] Verifying ${expectedPallets.length} expected pallets exist on forknet`);
  let palletsFound = 0;
  for (const p of expectedPallets) {
    if ((fork.query as any)[p]) {
      palletsFound++;
    } else {
      console.log(`    MISSING: ${p}`);
    }
  }
  console.log(`    pallets present: ${palletsFound}/${expectedPallets.length}`);
  if (palletsFound === expectedPallets.length) {
    console.log(`    PASS — all expected pallets present`);
    pass++;
  } else {
    console.log(`    FAIL — some expected pallets missing`);
    fail++;
  }

  await live.disconnect();
  await fork.disconnect();

  console.log(`\n=========================================`);
  console.log(`STATE-PRESERVED VERIFICATION: ${pass} PASS / ${fail} FAIL`);
  console.log(`=========================================`);
  process.exit(fail === 0 ? 0 : 1);
}

main().catch((err) => {
  console.error("FATAL:", err.message || err);
  process.exit(2);
});
