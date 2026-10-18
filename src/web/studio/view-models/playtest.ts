import { ref, reactive, computed } from "vue";
import { StudioApi } from "../api.ts";
import type { PlaytestSnapshot, DirectorEvaluation } from "../contracts.ts";
import { notify, studioState } from "../state.ts";

export function usePlaytestViewModel() {
  const activeTab = ref<"state" | "director" | "causality">("state");
  const sessions = ref<Array<{ session_id: string; turn: number; revision: number; current_location?: string }>>([]);
  const directorEval = ref<DirectorEvaluation | null>(null);
  const events = ref<any[]>([]);
  const loading = ref(false);

  const snapshot = computed(() => studioState.activePlaytestSnapshot);
  const sessionId = computed(() => studioState.activePlaytestSessionId);

  async function loadSessions() {
    try {
      sessions.value = await StudioApi.listPlaytestSessions();
    } catch (e) {
      // Ignored
    }
  }

  async function selectSession(id: string) {
    loading.value = true;
    try {
      studioState.activePlaytestSessionId = id;
      studioState.activePlaytestSnapshot = await StudioApi.getPlaytestSnapshot(id);
      await Promise.all([refreshDirector(), refreshEvents()]);
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    } finally {
      loading.value = false;
    }
  }

  async function refreshDirector() {
    if (!sessionId.value) return;
    try {
      directorEval.value = await StudioApi.evaluateDirector(sessionId.value);
    } catch (e) {
      // Ignored
    }
  }

  async function refreshEvents() {
    if (!sessionId.value) return;
    try {
      const res = await StudioApi.getPlaytestEvents(sessionId.value);
      events.value = res.events;
    } catch (e) {
      // Ignored
    }
  }

  async function waitTurn() {
    if (!sessionId.value || !snapshot.value) return;
    try {
      const res: any = await StudioApi.submitPlaytestCommand(sessionId.value, {
        idempotency_key: `cmd_${Date.now()}`,
        expected_revision: snapshot.value.world.revision,
        actor_id: "player",
        action: "wait",
        effects: [],
        advance_turn: true,
      });
      studioState.activePlaytestSnapshot!.world = res.world;
      await Promise.all([refreshDirector(), refreshEvents()]);
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    }
  }

  async function moveToLocation(locId: string) {
    if (!sessionId.value || !snapshot.value) return;
    try {
      const res: any = await StudioApi.submitPlaytestCommand(sessionId.value, {
        idempotency_key: `cmd_${Date.now()}`,
        expected_revision: snapshot.value.world.revision,
        actor_id: "player",
        action: "move",
        effects: [{ kind: "move_player", location_id: locId }],
        advance_turn: true,
      });
      studioState.activePlaytestSnapshot!.world = res.world;
      await Promise.all([refreshDirector(), refreshEvents()]);
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    }
  }

  async function chooseOption(storyletId: string, choiceId: string) {
    if (!sessionId.value || !snapshot.value) return;
    try {
      const res: any = await StudioApi.submitPlaytestChoice(
        sessionId.value,
        storyletId,
        choiceId,
        snapshot.value.world.revision,
        `choice_${Date.now()}`
      );
      studioState.activePlaytestSnapshot!.world = res.world;
      await Promise.all([refreshDirector(), refreshEvents()]);
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    }
  }

  async function forkPlaytest(srcSessionId: string, eventId: string) {
    try {
      const res = await StudioApi.forkPlaytest(srcSessionId, eventId);
      notify(`Forked session '${res.sessionId.slice(0, 8)}'`, "success");
      return res;
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
      return null;
    }
  }


  return {
    activeTab,
    sessions,
    directorEval,
    events,
    loading,
    snapshot,
    sessionId,
    loadSessions,
    selectSession,
    refreshDirector,
    refreshEvents,
    waitTurn,
    moveToLocation,
    chooseOption,
    forkPlaytest,
  };
}
