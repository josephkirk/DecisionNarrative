import { reactive } from "vue";
import { StudioApi, type RunsApi } from "../api.ts";
import type {
  InspectView, NearMiss, RunAction, RunEnvelope, RunStartInput,
} from "../contracts.ts";
import { choiceForKey, moveForKey } from "../runModel.ts";

export type { RunsApi };

export interface PlayState {
  runId: string | null;
  envelope: RunEnvelope | null;
  activeParticipant: number;
  inspectOpen: boolean;
  inspectTab: "knows" | "world" | "story";
  inspect: InspectView | null;
  whyOpen: boolean;
  nearMisses: NearMiss[];
  busy: boolean;
  error: string | null;
  reloadNote: string | null;
}

function message(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

export function usePlay(api: RunsApi = StudioApi.runs) {
  const state = reactive<PlayState>({
    runId: null, envelope: null, activeParticipant: 0, inspectOpen: false, inspectTab: "knows",
    inspect: null, whyOpen: false, nearMisses: [], busy: false, error: null, reloadNote: null,
  });
  let awaitingMove = false;

  /** Run one in-flight operation; a second call while busy is ignored. */
  async function guarded(op: () => Promise<void>): Promise<void> {
    if (state.busy) return;
    state.busy = true;
    state.error = null;
    try {
      await op();
    } catch (e) {
      state.error = message(e);
    } finally {
      state.busy = false;
    }
  }

  async function refreshPanels(): Promise<void> {
    if (!state.runId) return;
    if (state.whyOpen) state.nearMisses = (await api.whyNot(state.runId, state.activeParticipant)).nearMisses;
    if (state.inspectOpen) state.inspect = await api.inspect(state.runId, state.activeParticipant);
  }

  async function adopt(envelope: RunEnvelope): Promise<void> {
    state.envelope = envelope;
    state.runId = envelope.id;
    await refreshPanels();
  }

  function currentRevision(): number {
    return state.envelope?.revision ?? 0;
  }

  async function start(input: RunStartInput): Promise<void> {
    await guarded(async () => {
      state.reloadNote = null;
      state.activeParticipant = 0;
      await adopt(await api.start(input));
    });
  }

  async function open(runId: string): Promise<void> {
    await guarded(async () => {
      state.reloadNote = null;
      state.activeParticipant = 0;
      state.runId = runId;
      await adopt(await api.view(runId, 0));
    });
  }

  async function act(action: RunAction): Promise<void> {
    const id = state.runId;
    if (!id) return;
    // One key per user intent; a retry of the same intent must reuse it, a new click gets a new one.
    const key = crypto.randomUUID();
    await guarded(async () => {
      await adopt(await api.act(id, state.activeParticipant, action, currentRevision(), key));
    });
  }

  async function rewind(step: number): Promise<void> {
    const id = state.runId;
    if (!id) return;
    const key = crypto.randomUUID();
    await guarded(async () => {
      await adopt(await api.rewind(id, step, currentRevision(), key));
    });
  }

  async function undo(): Promise<void> {
    const step = state.envelope?.view.currentStep ?? 0;
    if (step > 0) await rewind(step - 1);
  }

  async function reload(): Promise<void> {
    const id = state.runId;
    if (!id) return;
    const total = state.envelope?.view.currentStep ?? 0;
    const key = crypto.randomUUID();
    await guarded(async () => {
      const result = await api.reload(id, currentRevision(), key);
      state.reloadNote = result.divergedAt === null
        ? `Replayed ${result.replayed} of ${total} steps; no divergence`
        : `Replayed ${result.replayed} of ${total} steps; diverged at step ${result.divergedAt}`;
      await adopt(result);
    });
  }

  async function toggleWhy(): Promise<void> {
    state.whyOpen = !state.whyOpen;
    if (!state.whyOpen || !state.runId) return;
    try {
      state.nearMisses = (await api.whyNot(state.runId, state.activeParticipant)).nearMisses;
    } catch (e) {
      state.error = message(e);
    }
  }

  function toggleInspect(): void {
    state.inspectOpen = !state.inspectOpen;
    if (!state.inspectOpen || !state.runId) return;
    api.inspect(state.runId, state.activeParticipant).then(
      (inspect) => { state.inspect = inspect; },
      (e) => { state.error = message(e); },
    );
  }

  async function setInspectTab(tab: PlayState["inspectTab"]): Promise<void> {
    state.inspectTab = tab;
    if (!state.runId) return;
    try {
      state.inspect = await api.inspect(state.runId, state.activeParticipant);
    } catch (e) {
      state.error = message(e);
    }
  }

  function setParticipant(id: number): void {
    const count = state.envelope?.view.participants.length ?? 0;
    if (!state.runId || id < 0 || id >= count) return;
    state.activeParticipant = id;
    // Switching who is playing re-reads the view; the latest selection wins if reads overlap.
    api.view(state.runId, id).then(
      (envelope) => {
        if (id === state.activeParticipant) void adopt(envelope);
      },
      (e) => { state.error = message(e); },
    );
  }

  function nextParticipant(dir: 1 | -1): void {
    const count = state.envelope?.view.participants.length ?? 0;
    if (count < 2) return;
    setParticipant((state.activeParticipant + dir + count) % count);
  }

  async function onKey(e: { key: string; ctrlKey: boolean }): Promise<void> {
    const view = state.envelope?.view;
    if (e.ctrlKey) {
      if (e.key.toLowerCase() === "z") await undo();
      return;
    }
    if (!view) return;
    if (awaitingMove) {
      awaitingMove = false;
      const move = /^[1-9]$/.test(e.key) ? moveForKey(view, Number(e.key)) : null;
      if (move) await act(move);
      return;
    }
    const key = e.key.toLowerCase();
    if (e.key === "Escape") {
      if (state.inspectOpen) state.inspectOpen = false;
      else if (state.whyOpen) state.whyOpen = false;
    } else if (key === "g") awaitingMove = true;
    else if (key === "?") await toggleWhy();
    else if (key === "i") toggleInspect();
    else if (key === "[") nextParticipant(-1);
    else if (key === "]") nextParticipant(1);
    else {
      const choice = choiceForKey(view, e.key);
      if (choice) await act(choice);
    }
  }

  return {
    state, start, open, act, rewind, undo, reload, toggleWhy, toggleInspect, setInspectTab, setParticipant, nextParticipant, onKey,
  };
}
