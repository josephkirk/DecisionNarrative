import { computed, ref } from "vue";
import { StudioApi } from "../api.ts";
import type { ChronoPlaytestSnapshot, ChronoPlaytestSessionSummary } from "../contracts.ts";
import { notify, studioState } from "../state.ts";

export function usePlaytestViewModel() {
  const activeTab = ref<"state" | "runtime" | "history">("runtime");
  const sessions = ref<ChronoPlaytestSessionSummary[]>([]);
  const loading = ref(false);
  const snapshot = computed(() => studioState.activeChronoPlaytestSnapshot);
  const sessionId = computed(() => studioState.activeChronoPlaytestSessionId);

  function applySnapshot(next: ChronoPlaytestSnapshot) {
    studioState.activeChronoPlaytestSessionId = next.session.sessionId;
    studioState.activeChronoPlaytestSnapshot = next;
    studioState.activePlaytestSessionId = null;
    studioState.activePlaytestSnapshot = null;
  }

  async function loadSessions() {
    try {
      sessions.value = await StudioApi.listChronoPlaytests();
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    }
  }

  async function selectSession(id: string) {
    if (!id) {
      studioState.activeChronoPlaytestSessionId = null;
      studioState.activeChronoPlaytestSnapshot = null;
      return;
    }
    loading.value = true;
    try {
      applySnapshot(await StudioApi.getChronoPlaytest(id));
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    } finally {
      loading.value = false;
    }
  }

  async function createSession(scenarioId?: string) {
    loading.value = true;
    try {
      const created = await StudioApi.createChronoPlaytest({
        sessionId: crypto.randomUUID(),
        ...(scenarioId ? { scenarioId } : {}),
      });
      applySnapshot(created);
      await loadSessions();
      notify("Started ChronoFlow playtest.", "success");
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    } finally {
      loading.value = false;
    }
  }

  async function updateSession(run: () => Promise<ChronoPlaytestSnapshot>) {
    if (!sessionId.value) return;
    loading.value = true;
    try {
      applySnapshot(await run());
      await loadSessions();
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
    } finally {
      loading.value = false;
    }
  }

  async function tick(frameBudgetMs = 16) {
    if (sessionId.value) {
      await updateSession(() => StudioApi.tickChronoPlaytest(sessionId.value!, frameBudgetMs));
    }
  }

  async function signal(eventName: string) {
    if (sessionId.value && eventName.trim()) {
      await updateSession(() => StudioApi.signalChronoPlaytest(sessionId.value!, eventName.trim()));
    }
  }

  async function chooseOutput(nodeGuid: number, outputPin: string) {
    if (sessionId.value) {
      await updateSession(() => StudioApi.chooseChronoPlaytest(sessionId.value!, { nodeGuid, outputPin }));
    }
  }

  async function chooseStorylet(nodeGuid: number, storyletId: string) {
    if (sessionId.value) {
      await updateSession(() => StudioApi.chooseChronoPlaytest(sessionId.value!, { nodeGuid, storyletId }));
    }
  }

  async function divertTopic(nodeGuid: number, edgeId: string) {
    if (sessionId.value) {
      await updateSession(() => StudioApi.divertChronoPlaytest(sessionId.value!, nodeGuid, edgeId));
    }
  }

  async function rollbackLatestCall() {
    const history = snapshot.value?.context.globalCallHistory;
    if (sessionId.value && history?.length) {
      await updateSession(() => StudioApi.reconcileChronoPlaytest(sessionId.value!, history.slice(0, -1)));
    }
  }

  return {
    activeTab,
    sessions,
    loading,
    snapshot,
    sessionId,
    loadSessions,
    selectSession,
    createSession,
    tick,
    signal,
    chooseOutput,
    chooseStorylet,
    divertTopic,
    rollbackLatestCall,
  };
}
