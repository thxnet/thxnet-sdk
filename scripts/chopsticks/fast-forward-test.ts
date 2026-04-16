// Chopsticks fast-forward test — drive N blocks across a session boundary and
// verify the runtime does not panic (pallet_aura::on_initialize or any other
// panicking on_session_change path).
//
// Usage:
//   # Start Chopsticks in another terminal:
//   bunx @acala-network/chopsticks -c scripts/chopsticks/leafchain-sand-testnet.yml \
//     -w target/release/wbuild/general-runtime/general_runtime.compact.compressed.wasm
//
//   # Run this test:
//   bun run scripts/chopsticks/fast-forward-test.ts \
//     --port 8102 --n-blocks 50 --chain leafchain-sand-testnet
//
// Why Chopsticks instead of try-runtime fast-forward?
//   try-runtime v0.10.1 fails on cumulus/AURA chains: its block header synthesis
//   does not inject a valid monotonically-increasing DigestItem::PreRuntime
//   (AURA_ENGINE_ID, slot), triggering the assertion at
//   substrate/frame/aura/src/lib.rs:133 ("Slot must increase").
//   Chopsticks drives real AURA block production with proper digests, so the
//   same session-boundary panic surface is exercised without the synthesis bug.
//
// ── Detection mechanism (three layers) ──────────────────────────────────────
//
// 1. PRIMARY — Block-advance check (fires in Phase 1 or Phase 2 depending on
//    injection timing):
//    After every dev_newBlock call, we query the chain header and assert
//    blockNow === prevBlockNumber + 1. When Chopsticks v1.3.0 encounters a WASM
//    runtime panic (e.g. pallet_aura::on_initialize panics), TxPool.buildBlock()
//    catches and swallows the panic
//    (chopsticks-core/dist/cjs/blockchain/txpool.js:240-244). The dev_newBlock
//    Promise resolves — but the block number does NOT advance. The block-advance
//    check detects this and emits "FAST-FORWARD FAILED: block number did not
//    advance after dev_newBlock".
//    Phase 1 applies this check once (after the unsafeBlockHeight jump); Phase 2
//    applies it each block in the careful window. With empty Authorities, the
//    panic fires at every block — so Phase 1's check is the earliest-fire point.
//
// 2. SECONDARY — Error-event scan (Phase 2, each block):
//    After every successful block, query system.events for ExtrinsicFailed or
//    sudo::Sudid(Err). These catch non-panic dispatch errors that do produce a
//    block but signal a runtime failure in an extrinsic result.
//
// 3. TERTIARY — Session-crossing assertion (post-loop):
//    Assert that at least one session boundary was crossed. A green result with
//    zero session crossings would be vacuous — it cannot exercise the
//    pallet_aura on_session_change path at all.
//
// ── Role of the try/catch on dev_newBlock ────────────────────────────────────
//    The try/catch wrapping each dev_newBlock call (below, Phase 2 loop) is
//    belt-and-suspenders for RPC-level errors: connection loss, invalid
//    params, or Chopsticks internal errors that propagate as rejected Promises.
//    It does NOT catch WASM runtime panics — Chopsticks swallows those in
//    TxPool.buildBlock() before the Promise resolves. The block-advance check
//    (layer 1 above) is the real runtime-panic detector.
//
// All failure exits emit a line prefixed with "FAST-FORWARD FAILED:" so CI
// log greps can surface the root cause in one command.

import { ApiPromise, WsProvider } from "@polkadot/api";

// ─── CLI flag parsing ────────────────────────────────────────────────────────
// Mirror upgrade-test.ts style: no library, pure process.argv scan.

const port = (() => {
  const idx = process.argv.indexOf("--port");
  return idx >= 0 ? parseInt(process.argv[idx + 1], 10) : 8102;
})();

const nBlocks = (() => {
  const idx = process.argv.indexOf("--n-blocks");
  return idx >= 0 ? parseInt(process.argv[idx + 1], 10) : 5;
})();

const chain = (() => {
  const idx = process.argv.indexOf("--chain");
  return idx >= 0 ? process.argv[idx + 1] : "leafchain-sand-testnet";
})();

// --fail-on-event: default true. Pass --no-fail-on-event to disable.
// Kept as a CLI surface so CI invocations self-document the intent.
const failOnEvent = !process.argv.includes("--no-fail-on-event");

// --try-state: opt-in, default false. NOT passed in CI.
// wasm-runtimes artifact is built without --features try-runtime; calling
// TryRuntime_execute_block on that WASM would fail with "unknown function".
const tryState = process.argv.includes("--try-state");

// ─── Session period ───────────────────────────────────────────────────────────
// Period = 6 * HOURS = 6 * 600 = 3600 blocks.
// Source: thxnet/leafchain/runtime/general/src/lib.rs:534
//   pub const Period: u32 = 6 * HOURS;   // HOURS = 600 (constants.rs:67)
const PERIOD = 3600;

// ─── 8-minute top-level watchdog ─────────────────────────────────────────────
// Prevents silent hang if Chopsticks stalls mid-run.
// Phase 1 uses unsafeBlockHeight (O(1) RPC call), Phase 2 drives only
// BOUNDARY_WINDOW + POST_BOUNDARY_BLOCKS = 10 blocks. Total wall time is
// well under 2 minutes for any live-fork position.
// Exit code 2 is distinct from exit code 1 (logic failure) so the caller can
// tell "hung" from "runtime error".
const WATCHDOG_MS = 8 * 60 * 1000;
const watchdog = setTimeout(() => {
  console.error(
    `FAST-FORWARD FAILED: watchdog — ${WATCHDOG_MS / 1000}s elapsed without completion`
  );
  process.exit(2);
}, WATCHDOG_MS);
// Allow Node/Bun to exit normally even if watchdog is still pending.
watchdog.unref();

// ─── Main ─────────────────────────────────────────────────────────────────────

const endpoint = `ws://localhost:${port}`;

async function main() {
  console.log(`[fast-forward] chain=${chain} port=${port} n-blocks=${nBlocks}`);
  console.log(`[fast-forward] fail-on-event=${failOnEvent} try-state=${tryState}`);
  console.log(`[fast-forward] Connecting to Chopsticks at ${endpoint}...`);

  // 2-minute RPC timeout per request. Phase 1 uses unsafeBlockHeight (single
  // block build, < 5s). Phase 2 drives 10 individual blocks (~15s total).
  // Default 60s is sufficient for individual calls; use 120s for headroom.
  const RPC_TIMEOUT_MS = 2 * 60 * 1000;
  const provider = new WsProvider(endpoint, undefined, undefined, RPC_TIMEOUT_MS);

  // Surface WebSocket-level disconnection as a named failure rather than an
  // unhandled rejection with a cryptic message.
  // intentionalDisconnect is set to true before we call api.disconnect() so
  // the handler does not fire exit(1) on a clean teardown.
  let intentionalDisconnect = false;
  provider.on("disconnected", () => {
    if (intentionalDisconnect) return;
    console.error(
      "FAST-FORWARD FAILED: connection error: WebSocket disconnected unexpectedly"
    );
    process.exit(1);
  });

  const api = await ApiPromise.create({ provider });

  // Helper: log a failure line, mark disconnect as intentional, then exit.
  // All error paths use this so the disconnect handler does not fire a
  // spurious "WebSocket disconnected unexpectedly" on clean teardown.
  const failExit = async (msg: string, code = 1): Promise<never> => {
    console.error(`FAST-FORWARD FAILED: ${msg}`);
    intentionalDisconnect = true;
    await api.disconnect().catch(() => {});
    process.exit(code);
  };

  const runtimeVersion = api.runtimeVersion;
  console.log(
    `[fast-forward] runtime: ${runtimeVersion.specName.toString()} v${runtimeVersion.specVersion.toNumber()}`
  );

  // ── Baselines (pre-loop) ──────────────────────────────────────────────────
  const headerBefore = await api.rpc.chain.getHeader();
  const blockBefore = headerBefore.number.toNumber();
  const stateRootBefore = headerBefore.stateRoot.toString();

  const sessionBefore = (await api.query.session.currentIndex()).toNumber();

  // ── Two-phase strategy ────────────────────────────────────────────────────
  // blocks_to_next_session = Period - (blockNumber % Period)
  // Edge case: if blockNumber % Period === 0, chain is exactly AT a boundary;
  // blocks_to_next_session = Period (3600).
  //
  // Phase 1 — bulk skip: drive blocks_to_next_session - BOUNDARY_WINDOW blocks
  //   via unsafeBlockHeight, then verify the jump landed correctly (block-advance
  //   check). NOTE: if Authorities is empty, pallet_aura::on_initialize executes
  //   slot % authorities.len() at EVERY block — not just at the session boundary
  //   — so a panic can be observed here too. The block-advance check after the
  //   Phase 1 jump is the PRIMARY detector for that case.
  // Phase 2 — careful window: drive BOUNDARY_WINDOW + POST_BOUNDARY_BLOCKS one-
  //   by-one with full monitoring (session tracking, event scan, block advance).
  //
  // This guarantees boundary crossing in O(1) + O(BOUNDARY_WINDOW) time
  // regardless of where the live fork lands.
  const BOUNDARY_WINDOW = 5; // blocks before boundary to switch to careful mode
  const POST_BOUNDARY_BLOCKS = nBlocks; // blocks after boundary; controlled by --n-blocks (default 5)
  const blocksToNextSession = PERIOD - (blockBefore % PERIOD);

  // Bulk-skip count: how many blocks to drive in Phase 1.
  // When blocksToNextSession <= BOUNDARY_WINDOW, we're already in the window —
  // skip Phase 1 entirely (bulkSkip = 0).
  const bulkSkip = Math.max(0, blocksToNextSession - BOUNDARY_WINDOW);
  const carefulCount = BOUNDARY_WINDOW + POST_BOUNDARY_BLOCKS; // Phase 2 blocks

  console.log(`[fast-forward] block_before=#${blockBefore}  session_before=${sessionBefore}`);
  console.log(
    `[fast-forward] blocks_to_next_session=${blocksToNextSession}  bulk_skip=${bulkSkip}  careful_count=${carefulCount}`
  );

  let prevBlockNumber = blockBefore;
  let prevStateRoot = stateRootBefore;
  let sessionCrossings = 0;
  let sessionAfter = sessionBefore;

  // ── Phase 1: Jump to boundary window via unsafeBlockHeight ───────────────
  // When blocksToNextSession > BOUNDARY_WINDOW, we use dev_newBlock with
  // unsafeBlockHeight to jump directly to (nextSessionBlock - BOUNDARY_WINDOW - 1),
  // then let Phase 2 drive the last BOUNDARY_WINDOW + POST_BOUNDARY_BLOCKS
  // blocks one-by-one.
  //
  // unsafeBlockHeight sets the block number in storage WITHOUT building all
  // intermediate blocks — effectively teleporting the chain state to the target
  // height. This bypasses the network round-trip cost of fetching relay chain
  // state for each intermediate block, reducing O(blocksToNextSession) time
  // to O(1).
  //
  // The session index in on-chain storage is NOT updated by this jump (sessions
  // are still at sessionBefore). Phase 2 drives the actual boundary crossing
  // to exercise the on_session_change path and detect any panic.
  if (bulkSkip > 0) {
    const jumpTarget = blockBefore + bulkSkip; // = nextSessionBlock - BOUNDARY_WINDOW - 1
    console.log(`[fast-forward] Phase 1: teleporting to block #${jumpTarget} via unsafeBlockHeight...`);
    try {
      await provider.send("dev_newBlock", [{ count: 1, unsafeBlockHeight: jumpTarget }]);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      await failExit(`unsafeBlockHeight jump to #${jumpTarget} rejected: ${msg}`);
    }
    const headerAfterJump = await api.rpc.chain.getHeader();
    const blockAfterJump = headerAfterJump.number.toNumber();
    if (blockAfterJump !== jumpTarget) {
      await failExit(
        `after unsafeBlockHeight jump expected block #${jumpTarget}, got #${blockAfterJump}`
      );
    }
    prevBlockNumber = blockAfterJump;
    prevStateRoot = headerAfterJump.stateRoot.toString();
    console.log(`[fast-forward] Phase 1 done: teleported to block #${prevBlockNumber}`);
  }

  // ── Phase 2: Careful window — one block at a time ─────────────────────────
  console.log(`[fast-forward] Phase 2: careful monitoring for ${carefulCount} blocks...`);

  for (let i = 0; i < carefulCount; i++) {
    const expectedBlock = prevBlockNumber + 1;

    // Belt-and-suspenders: catch RPC-level errors from dev_newBlock.
    // This try/catch fires for: WebSocket errors, Chopsticks internal errors
    // that propagate as rejected Promises, invalid parameter errors.
    //
    // IMPORTANT: this does NOT catch WASM runtime panics.
    // Chopsticks v1.3.0 swallows WASM panics in TxPool.buildBlock()
    // (chopsticks-core/dist/cjs/blockchain/txpool.js:240-244) — the build
    // silently fails, the Promise resolves successfully, but the block number
    // does NOT advance. The block-advance check below (PRIMARY detector) is
    // what catches runtime panics.
    try {
      await provider.send("dev_newBlock", [{ count: 1 }]);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      await failExit(`dev_newBlock rejected at block #${expectedBlock}: ${msg}`);
    }

    // ── PRIMARY runtime-panic detector: block-number advance check ───────────
    // Chopsticks v1.3.0 swallows WASM panics silently (see comment above).
    // When a panic occurs, dev_newBlock resolves but the block is not committed
    // — the chain head stays frozen. Asserting blockNow === prevBlockNumber + 1
    // is the only reliable way to detect a swallowed WASM panic.
    // Any mismatch emits "FAST-FORWARD FAILED: block number did not advance..."
    const headerNow = await api.rpc.chain.getHeader();
    const blockNow = headerNow.number.toNumber();
    const stateRootNow = headerNow.stateRoot.toString();

    if (blockNow !== expectedBlock) {
      await failExit(
        `block number did not advance after dev_newBlock — expected #${expectedBlock}, got #${blockNow}`
      );
    }

    // ── stateRoot advance check (warn only) ───────────────────────────────
    if (stateRootNow === prevStateRoot) {
      console.warn(
        `WARNING: stateRoot unchanged at block #${blockNow} — block may be empty (no inherents/extrinsics processed)`
      );
    }

    // ── Session index tracking ─────────────────────────────────────────────
    const sessionNow = (await api.query.session.currentIndex()).toNumber();
    if (sessionNow > sessionAfter) {
      console.log(
        `SESSION BOUNDARY crossed at block #${blockNow}: session index ${sessionAfter} → ${sessionNow}`
      );
      sessionCrossings += sessionNow - sessionAfter;
      sessionAfter = sessionNow;
    }

    // ── Error-event scan ──────────────────────────────────────────────────
    // Query system events at the new block's hash. Look for:
    //   - ExtrinsicFailed (system pallet)
    //   - Sudid with Err variant (sudo pallet)
    if (failOnEvent) {
      const blockHash = await api.rpc.chain.getBlockHash(blockNow);
      const events = await api.query.system.events.at(blockHash);

      for (const record of events) {
        const { event } = record;
        const section = event.section.toString();
        const method = event.method.toString();

        // system::ExtrinsicFailed
        if (section === "system" && method === "ExtrinsicFailed") {
          await failExit(
            `error event in block #${blockNow}: ${section}::${method} — ${event.data.toString()}`
          );
        }

        // sudo::Sudid with Err variant
        if (section === "sudo" && method === "Sudid") {
          // event.data[0] is the DispatchResult; check if it's an Err
          const dispatchResult = event.data[0];
          if (dispatchResult && dispatchResult.toString().includes("Err")) {
            await failExit(
              `error event in block #${blockNow}: ${section}::${method} with Err — ${event.data.toString()}`
            );
          }
        }
      }
    }

    // ── Per-block progress log ─────────────────────────────────────────────
    const progress = `[${i + 1}/${carefulCount}]`;
    console.log(
      `${progress} block #${blockNow}  session=${sessionNow}  stateRoot=${stateRootNow.slice(0, 12)}...`
    );

    prevBlockNumber = blockNow;
    prevStateRoot = stateRootNow;
  }

  // ── Post-loop summary ─────────────────────────────────────────────────────
  const blockAfter = prevBlockNumber;
  console.log(
    `\n[fast-forward] completed: drove blocks #${blockBefore + 1}–#${blockAfter}`
  );
  console.log(
    `[fast-forward] session: ${sessionBefore} → ${sessionAfter}  (${sessionCrossings} crossing(s))`
  );

  // Session-crossing assertion: the test is only meaningful evidence if
  // at least one session boundary was exercised. Without a crossing, the
  // gate cannot detect the pallet_aura on_initialize panic that fires at
  // the session boundary — a green result would be vacuous.
  if (sessionCrossings < 1) {
    await failExit(
      `no session boundary crossed` +
        ` (next session boundary was at block #${blockBefore + blocksToNextSession},` +
        ` drove from #${blockBefore + 1} to #${blockAfter})` +
        ` — this should not happen with the two-phase strategy; check PERIOD constant`
    );
  }

  console.log("\n=== FAST-FORWARD TEST PASSED ===");
  intentionalDisconnect = true;
  await api.disconnect();
  process.exit(0);
}

main().catch((err: unknown) => {
  const msg = err instanceof Error ? err.message : String(err);
  console.error(`FAST-FORWARD FAILED: unhandled error — ${msg}`);
  process.exit(1);
});
