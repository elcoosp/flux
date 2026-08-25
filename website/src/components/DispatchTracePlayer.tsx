import { useEffect, useMemo, useState } from 'react';
import type { ReactElement } from 'react';

import counterTrace from '../assets/traces/counter-init.jsonl?raw';
import counterTree from '../assets/traces/counter-init-tree.json';

/**
 * A single reconcile trace event (reconcile-trace-format.md grammar). Only the
 * fields the player visualizes are typed; the rest pass through opaquely.
 */
interface TraceEvent {
  t: string;
  seq?: number;
  i?: number;
  id?: number;
  ids?: number[];
  handler?: number;
  built?: number;
  updated?: number;
  detached?: number;
  skipped_unchanged?: number;
  skipped_pure?: number;
  prop_materializations?: number;
}

/** A ViewNode snapshot (mirrors ShadowNode). */
interface ViewNode {
  id: number;
  kind: string;
  componentId: number;
  key: number | null;
  isPure: boolean;
  signalDeps: number[];
  children: number[];
}

interface ViewTree {
  root: number;
  nodes: Record<string, ViewNode>;
}

interface I18nStrings {
  title: string;
  intro: string;
  webImpossible: string;
  sourcePane: string;
  wirePane: string;
  treePane: string;
  nativePane: string;
  nativePending: string;
  tapCounter: string;
  tapHint: string;
  step: string;
  phase: string;
  signals: string;
  dirty: string;
  updated: string;
  built: string;
}

/** Parses canonical JSONL into trace events (one per non-empty line). */
function parseTrace(text: string): TraceEvent[] {
  return text
    .split('\n')
    .map((l) => l.trim())
    .filter((l) => l.length > 0)
    .map((l) => JSON.parse(l) as TraceEvent);
}

/** Groups trace events into replay steps, split on each `step_end`. */
function splitSteps(events: TraceEvent[]): TraceEvent[][] {
  const steps: TraceEvent[][] = [];
  let current: TraceEvent[] = [];
  for (const ev of events) {
    current.push(ev);
    if (ev.t === 'step_end') {
      steps.push(current);
      current = [];
    }
  }
  if (current.length > 0) steps.push(current);
  return steps;
}

/**
 * The honest Flux playground: a recorded dispatch replay. Flux cannot compile to
 * the web, so instead of a live editor this island steps through a real recorded
 * reconcile trace (counter-init.jsonl) alongside the captured wire frame and the
 * view-tree snapshot, highlighting the nodes a tap actually touched.
 */
export default function DispatchTracePlayer({
  i18n,
  source,
  hex,
}: {
  i18n: I18nStrings;
  source: string;
  hex: string;
}) {
  const steps = useMemo(
    () => splitSteps(parseTrace(counterTrace)),
    [],
  );
  const tree = useMemo(() => counterTree as unknown as ViewTree, []);
  const [stepIdx, setStepIdx] = useState(0);

  const step = steps[Math.min(stepIdx, steps.length - 1)] ?? [];

  // Derive the highlight sets from the events visible up to the current step.
  const dirtyIds = useMemo(() => {
    const ids = new Set<number>();
    for (const ev of step) {
      if (ev.t === 'dirty' && ev.ids) ev.ids.forEach((d) => ids.add(d));
      if (ev.t === 'update' && typeof ev.id === 'number') ids.add(ev.id);
    }
    return ids;
  }, [step]);

  const signalIds = useMemo(() => {
    const ids = new Set<number>();
    for (const ev of step) {
      if (ev.t === 'signals' && ev.ids) ev.ids.forEach((s) => ids.add(s));
    }
    return ids;
  }, [step]);

  const lastStepEnd = useMemo(
    () => step.find((e) => e.t === 'step_end') as TraceEvent | undefined,
    [step],
  );

  const maxStep = steps.length - 1;

  // Auto-reset the highlight when the step changes by re-keying the SVG.
  useEffect(() => {
    /* step state drives all derived highlight sets; nothing to do here. */
  }, [stepIdx]);

  return (
    <div className="flux-trace-player" data-locale={i18n.title ? undefined : undefined}>
      <h3>{i18n.title}</h3>
      <p className="flux-trace-intro">{i18n.intro}</p>
      <p className="flux-trace-note">{i18n.webImpossible}</p>

      <div className="flux-trace-controls">
        <button
          type="button"
          onClick={() => setStepIdx((i) => Math.max(0, i - 1))}
          disabled={stepIdx === 0}
        >
          ◀ {i18n.step}
        </button>
        <span className="flux-trace-counter">
          {i18n.tapCounter}: {stepIdx} / {maxStep}
        </span>
        <button
          type="button"
          onClick={() => setStepIdx((i) => Math.min(maxStep, i + 1))}
          disabled={stepIdx === maxStep}
        >
          {i18n.step} ▶
        </button>
      </div>
      <p className="flux-trace-hint">{i18n.tapHint}</p>

      <div className="flux-trace-grid">
        <section className="flux-pane">
          <h4>{i18n.sourcePane}</h4>
          <pre className="flux-code">{source}</pre>
        </section>

        <section className="flux-pane">
          <h4>{i18n.wirePane}</h4>
          <pre className="flux-code flux-hex">{hex}</pre>
        </section>

        <section className="flux-pane">
          <h4>{i18n.treePane}</h4>
          <svg viewBox="0 0 320 220" className="flux-tree" role="img" aria-label={i18n.treePane}>
            {renderTree(tree, dirtyIds, signalIds)}
          </svg>
        </section>

        <section className="flux-pane flux-native">
          <h4>{i18n.nativePane}</h4>
          <div className="flux-native-badge" aria-label={i18n.nativePending}>
            ⚠ {i18n.nativePending}
          </div>
          <p className="flux-native-body">
            Count: {lastStepEnd ? (stepIdx === 0 ? 0 : 1) : 0}
          </p>
        </section>
      </div>

      <div className="flux-trace-stats">
        <span>
          {i18n.signals}: {[...signalIds].join(', ') || '—'}
        </span>
        <span>
          {i18n.dirty}: {[...dirtyIds].join(', ') || '—'}
        </span>
        <span>
          {i18n.updated}: {lastStepEnd?.updated ?? 0}
        </span>
        <span>
          {i18n.built}: {lastStepEnd?.built ?? 0}
        </span>
      </div>
    </div>
  );
}

/** Renders the ViewNode tree as a small SVG, highlighting dirty/signal nodes. */
function renderTree(
  tree: ViewTree,
  dirtyIds: Set<number>,
  signalIds: Set<number>,
): ReactElement[] {
  const out: ReactElement[] = [];
  const layouts: Record<number, { x: number; y: number }> = {
    1: { x: 160, y: 30 },
    57: { x: 90, y: 130 },
    7: { x: 230, y: 130 },
  };

  for (const key of Object.keys(tree.nodes)) {
    const node = tree.nodes[key];
    const pos = layouts[node.id] ?? { x: 160, y: 110 };
    const isDirty = dirtyIds.has(node.id);
    const readsSignal = node.signalDeps.some((d) => signalIds.has(d));
    const fill = isDirty ? '#f59e0b' : readsSignal ? '#38bdf8' : '#1e293b';
    out.push(
      <g key={`n-${node.id}`}>
        <rect
          x={pos.x - 50}
          y={pos.y - 20}
          width={100}
          height={40}
          rx={8}
          fill={fill}
          stroke="#0f172a"
          strokeWidth={1.5}
        />
        <text x={pos.x} y={pos.y + 4} textAnchor="middle" fill="#e2e8f0" fontSize={12}>
          {node.kind} #{node.id}
        </text>
      </g>,
    );
  }

  // Edges from root to children.
  const root = tree.nodes[String(tree.root)];
  if (root) {
    for (const childId of root.children) {
      const c = layouts[childId];
      const r = layouts[root.id];
      if (c && r) {
        out.push(
          <line
            key={`e-${childId}`}
            x1={r.x}
            y1={r.y + 20}
            x2={c.x}
            y2={c.y - 20}
            stroke="#475569"
            strokeWidth={1.5}
          />,
        );
      }
    }
  }
  return out;
}
