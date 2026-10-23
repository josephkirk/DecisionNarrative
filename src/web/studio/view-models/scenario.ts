import { ref, reactive, watch } from "vue";
import { StudioApi } from "../api.ts";
import type { ScenarioRecord, ResolvedScenarioStart } from "../contracts.ts";
import { notify, studioState } from "../state.ts";

export function useScenarioViewModel() {
  const scenarios = ref<ScenarioRecord[]>([]);
  const selectedScenarioId = ref<string>("");
  const preview = ref<ResolvedScenarioStart | null>(null);
  const loading = ref(false);
  const playtestError = ref<string | null>(null);

  const activeScenario = reactive<ScenarioRecord>({
    id: "default-scenario",
    name: "Default Scenario",
    seed: 1,
    start_location_policy: { mode: "fixed", location_id: "" },
    initial_qualities: {},
    actor_locations: {},
    actor_population_policy: {
      mode: "all",
      location_policy: { mode: "fixed" },
    },
  });

  async function loadScenarios() {
    loading.value = true;
    try {
      scenarios.value = await StudioApi.listScenarios();
      if (scenarios.value.length > 0 && !selectedScenarioId.value) {
        selectScenario(scenarios.value[0]);
      }
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    } finally {
      loading.value = false;
    }
  }

  function selectScenario(scen: ScenarioRecord) {
    selectedScenarioId.value = scen.id;
    Object.assign(activeScenario, JSON.parse(JSON.stringify(scen)));
    triggerPreview();
  }

  let debounceTimer: any = null;
  function triggerPreview() {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(async () => {
      try {
        preview.value = await StudioApi.previewScenario(activeScenario);
      } catch (e) {
        // Ignored during typing
      }
    }, 250);
  }

  async function saveScenario() {
    try {
      await StudioApi.saveContent("scenarios", activeScenario);
      notify(`Saved scenario '${activeScenario.name}'`, "success");
      await loadScenarios();
    } catch (e) {
      notify(e instanceof Error ? e.message : String(e), "error");
    }
  }

  /** Replace authored ids in engine messages with the names the user sees. */
  function humanizeScenarioError(message: string): string {
    const names = [
      ...studioState.entities.map((entity) => ({ id: entity.id, name: entity.name })),
      ...studioState.scenarios.map((scenario) => ({ id: scenario.id, name: scenario.name })),
    ]
      .filter((entry) => entry.id)
      .sort((left, right) => right.id.length - left.id.length);
    let text = message;
    for (const entry of names) {
      text = text.split(entry.id).join(`'${entry.name}'`);
    }
    return text;
  }

  async function startPlaytestFromScenario() {
    playtestError.value = null;
    try {
      const res = await StudioApi.createChronoPlaytest({
        sessionId: crypto.randomUUID(),
        scenarioId: activeScenario.id,
      });
      studioState.activeChronoPlaytestSessionId = res.session.sessionId;
      studioState.activeChronoPlaytestSnapshot = res;
      studioState.activePlaytestSessionId = null;
      studioState.activePlaytestSnapshot = null;
      notify(`Started ChronoFlow run '${res.session.sessionId.slice(0, 8)}'`, "success");
      return res.session.sessionId;
    } catch (e) {
      const message = humanizeScenarioError(e instanceof Error ? e.message : String(e));
      playtestError.value = message;
      notify(message, "error");
      return null;
    }
  }

  return {
    scenarios,
    selectedScenarioId,
    activeScenario,
    preview,
    loading,
    loadScenarios,
    selectScenario,
    triggerPreview,
    saveScenario,
    startPlaytestFromScenario,
    playtestError,
  };
}
