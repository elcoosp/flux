/**
 * make-fixtures.ts
 *
 * Produces the COMMITTED FIXTURE assets the DispatchTracePlayer replays, so the
 * playground works without the runtime `cargo test --features trace-dump`
 * feature (which is owned by the runtime/parity agents — FA-DOCS must NOT
 * invent it).
 *
 * The fixtures are hand-authored to match the normative formats exactly:
 *  - `counter-init.jsonl` : a reconcile trace in the grammar of
 *    `docs/spec/reconcile-trace-format.md` (the `counter_1000` / `noop_dispatch`
 *    golden shapes), using the canonical JSONL line form (sorted keys, no
 *    whitespace) as emitted by `TraceEvent.toJsonLine()` on Android and the
 *    equivalent Swift sink.
 *  - `counter-init.hex`    : a hex dump of an Appendix-D `Init` frame
 *    (magic `0x465C5558`, version 1, full-tree flag) for the FrameInspector.
 *  - `counter-init-tree.json` : a ViewNode tree snapshot (mirrors the
 *    `ShadowNode` shape from `runtimes/.../shadow/ShadowNode.kt`).
 *
 * When FA-ANDROID / FLUX-023 land `trace-dump`, the CI script
 * `scripts/generate-trace-assets.ts` should replace this hand authoring by
 * running `cargo test --features trace-dump` and copying the emitted JSONL/hex.
 * Until then these fixtures are the source of truth for the demo.
 *
 * Run with: `pnpm make:fixtures`
 */
import { mkdir, writeFile } from 'node:fs/promises';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const assetsDir = join(__dirname, '..', 'src', 'assets', 'traces');

const FLUX_MAGIC = 0x465c5558; // "FLUX" little-endian

/** Canonical JSONL line: sorted keys, no whitespace (matches TraceEvent output). */
function canonicalLine(obj: Record<string, unknown>): string {
  const keys = Object.keys(obj).sort();
  const parts = keys.map((k) => {
    const v = obj[k];
    if (v === null) return `"${k}":null`;
    return `"${k}":${typeof v === 'string' ? JSON.stringify(v) : String(v)}`;
  });
  return `{${parts.join(',')}}`;
}

/**
 * Builds the counter reconcile trace. Models the ADR-0027 Phase-1 + R1 dirty-set
 * walk: a tap writes signal 1, the Text bound to it is the only dirty node, one
 * `update`, zero builds. Mirrors the `counter_1000` golden assertion.
 */
function buildCounterTrace(): string {
  return [
    canonicalLine({ t: 'frame', seq: 0, full: true, root: 1, nodes: 4, patches: 3 }),
    canonicalLine({ t: 'apply_patch', seq: 0, patches: 3 }),
    canonicalLine({ t: 'step_end', i: 0, built: 4, updated: 0, skipped_unchanged: 0, skipped_pure: 0, detached: 0, prop_materializations: 8 }),
    canonicalLine({ t: 'dispatch', seq: 1, handler: 7 }),
    canonicalLine({ t: 'signals', seq: 1, ids: [1] }),
    canonicalLine({ t: 'dirty', seq: 1, ids: [57] }),
    canonicalLine({ t: 'update', seq: 1, id: 57 }),
    canonicalLine({ t: 'step_end', i: 1, built: 0, updated: 1, skipped_unchanged: 0, skipped_pure: 0, detached: 0, prop_materializations: 2 }),
  ].join('\n') + '\n';
}

/**
 * Encodes an Appendix-D Init frame as bytes and returns its hex form with
 * byte-offset annotations handled by the FrameInspector (here we emit just the
 * raw hex; the inspector knows the layout). Layout per D.1 + D.2/D.3:
 *   magic(4) version(1) seq(4) flags(1) patch_count(2) handler_count(2)
 *   string_count(2) ... then a single Replace patch (tag 0x01) carrying the
 *   root Node (id=1, kind=1 Primitive, componentId=0, 1 prop, 2 children).
 */
function buildInitFrameHex(): string {
  const buf: number[] = [];
  // D.1 Frame header
  const magic = FLUX_MAGIC >>> 0;
  buf.push(magic & 0xff, (magic >>> 8) & 0xff, (magic >>> 16) & 0xff, (magic >>> 24) & 0xff);
  buf.push(0x01); // version 1
  // seq = 0  (little-endian u32)
  buf.push(0x00, 0x00, 0x00, 0x00);
  // flags = 0x01 (full_tree)
  buf.push(0x01);
  // patch_count = 1 (u16 LE)
  buf.push(0x01, 0x00);
  // handler_count = 1 (u16 LE) — the counter's tap handler id 7
  buf.push(0x01, 0x00);
  // string_count = 1 (u16 LE) — one interned string ("count")
  buf.push(0x01, 0x00);

  // D.2 Patch: Replace (tag 0x01) = u32 id, Node
  buf.push(0x01);
  // D.3 Node for the root Column
  buf.push(0x01, 0x00, 0x00, 0x00); // id = 1
  buf.push(0x01); // kind = 1 (Primitive)
  buf.push(0x00, 0x00, 0x00, 0x00); // component_id = 0 (Column)
  buf.push(0x00, 0x01); // prop_count = 1 (u16 LE)
  // prop: (u16 prop_idx=0, Value: tag 0x04 Str => u32 string_id=0)
  buf.push(0x00, 0x00, 0x04, 0x00, 0x00, 0x00, 0x00);
  buf.push(0x02, 0x00); // child_count = 2 (u16 LE)
  // child 1: Node (tag 0x01) u32 id=57 (Text "Count: 0")
  buf.push(0x01, 0x39, 0x00, 0x00, 0x00);
  // child 2: Node (tag 0x01) u32 id=7 (Button "Increment")
  buf.push(0x01, 0x07, 0x00, 0x00, 0x00);
  buf.push(0x00, 0x00); // handler_count = 0
  // span: file(4) start(4) end(4) = 0,0,0
  buf.push(0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00);

  // HandlerDef (id=7, closure ref placeholder): id(4) + closure hash(8)=0 + offsets(...)
  buf.push(0x07, 0x00, 0x00, 0x00);
  for (let i = 0; i < 8 + 4 + 2 + 2 + 12; i++) buf.push(0x00);

  // StringEntry: id(4)=0, len(2)=5, bytes "count"
  buf.push(0x00, 0x00, 0x00, 0x00, 0x05, 0x00);
  buf.push(0x63, 0x6f, 0x75, 0x6e, 0x74); // "count"

  return Buffer.from(buf).toString('hex');
}

/** ViewNode tree snapshot (mirrors ShadowNode fields: id, kind, componentId, key, isPure, signalDeps, children). */
function buildViewTree(): string {
  const tree = {
    root: 1,
    nodes: {
      '1': { id: 1, kind: 'column', componentId: 0, key: null, isPure: false, signalDeps: [], children: [57, 7] },
      '57': { id: 57, kind: 'text', componentId: 1, key: null, isPure: false, signalDeps: [1], children: [] },
      '7': { id: 7, kind: 'button', componentId: 2, key: null, isPure: false, signalDeps: [], children: [] },
    },
  };
  return JSON.stringify(tree, null, 2);
}

async function main(): Promise<void> {
  await mkdir(assetsDir, { recursive: true });
  await writeFile(join(assetsDir, 'counter-init.jsonl'), buildCounterTrace(), 'utf8');
  await writeFile(join(assetsDir, 'counter-init.hex'), buildInitFrameHex(), 'utf8');
  await writeFile(join(assetsDir, 'counter-init-tree.json'), buildViewTree(), 'utf8');
  console.log(`make-fixtures: wrote fixtures to ${assetsDir}`);
  console.log('  - counter-init.jsonl  (reconcile trace, counter_1000 shape)');
  console.log('  - counter-init.hex    (Appendix D Init frame)');
  console.log('  - counter-init-tree.json (ViewNode snapshot)');
}

const isMain =
  process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1];
if (isMain) {
  await main();
}
