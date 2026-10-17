<script setup lang="ts">
import { onMounted, ref, reactive, computed } from "vue";
import { useGenerationViewModel } from "../view-models/generation.ts";
import { studioState } from "../state.ts";

const {
  jobs,
  selectedJobId,
  loading,
  loadJobs,
  createJob,
  advanceStage,
  editStage,
  saveDraft,
} = useGenerationViewModel();
const newJob = reactive({
  title: "New Staged Scene",
  location: "",
  clusterType: "player_controlled_passage",
  characterRoles: ["witness"],
  triggerPreconditions: {},
  desiredOutcomes: {},
});

const selectedStageId = ref<string>("canon_and_voice");
const stageEditText = ref<string>("");

onMounted(async () => {
  await loadJobs();
});

const currentJob = computed(() => {
  return jobs.value.find((j) => j.id === selectedJobId.value) || null;
});

function handleSelectJob(job: any) {
  selectedJobId.value = job.id;
  const lastStage = job.stages?.[job.stages.length - 1];
  if (lastStage) {
    selectedStageId.value = lastStage.stage;
    stageEditText.value = JSON.stringify(lastStage.artifact, null, 2);
  }
}

function selectStage(stageRec: any) {
  selectedStageId.value = stageRec.stage;
  stageEditText.value = JSON.stringify(stageRec.artifact, null, 2);
}

async function handleCreateJob() {
  await createJob({
    title: newJob.title,
    location: newJob.location || studioState.locations[0]?.id || "platform",
    clusterType: newJob.clusterType,
    characterRoles: newJob.characterRoles,
    triggerPreconditions: newJob.triggerPreconditions,
    desiredOutcomes: newJob.desiredOutcomes,
  });
}

async function handleSaveStageEdit() {
  if (!currentJob.value) return;
  try {
    const artifact = JSON.parse(stageEditText.value);
    await editStage(currentJob.value.id, selectedStageId.value, currentJob.value.revision, artifact);
  } catch (e) {
    alert("Invalid JSON format for stage artifact");
  }
}
</script>

<template>
  <div style="display:flex; flex-direction:column; height:100%;">
    <header class="view-header">
      <h1 class="view-title">Strata Generation Pipeline</h1>
      <div v-if="currentJob" style="display:flex; align-items:center; gap:8px;">
        <span class="badge">{{ currentJob.status }} · Stage {{ currentJob.next_stage_index }}/9</span>
        <button
          class="btn btn-primary"
          :disabled="currentJob.status === 'complete' || currentJob.status === 'running'"
          @click="advanceStage(currentJob.id, currentJob.revision)"
        >
          Advance Next Stage
        </button>
        <button
          v-if="currentJob.status === 'complete'"
          class="btn btn-secondary"
          @click="saveDraft(currentJob.id)"
        >
          Save Draft to Storylets
        </button>
      </div>
    </header>

    <div style="display:flex; flex:1; overflow:hidden;">
      <!-- Left: Create Form & Existing Jobs List -->
      <div style="width:340px; border-right:1px solid var(--border-subtle); display:flex; flex-direction:column; background:var(--bg-card);">
        <div style="padding:16px; border-bottom:1px solid var(--border-subtle);">
          <h3 style="font-size:13px; font-weight:700; margin-bottom:10px;">NEW GENERATION JOB</h3>
          <div class="form-group" style="margin-bottom:8px;">
            <label class="form-label">Scene Title</label>
            <input v-model="newJob.title" class="form-input" />
          </div>
          <div class="form-group" style="margin-bottom:10px;">
            <label class="form-label">Location</label>
            <select v-model="newJob.location" class="form-select">
              <option v-for="l in studioState.locations" :key="l.id" :value="l.id">{{ l.name }}</option>
            </select>
          </div>
          <button class="btn btn-primary" style="width:100%;" @click="handleCreateJob">+ Create Staged Job</button>
        </div>

        <div style="flex:1; overflow-y:auto; padding:12px;">
          <h4 style="font-size:11px; font-weight:700; color:var(--text-muted); margin-bottom:8px; text-transform:uppercase;">Recent Jobs</h4>
          <div
            v-for="job in jobs"
            :key="job.id"
            :style="{
              padding: '10px 12px',
              borderRadius: '8px',
              marginBottom: '6px',
              cursor: 'pointer',
              background: job.id === selectedJobId ? 'var(--accent-subtle)' : 'var(--bg-input)',
              border: job.id === selectedJobId ? '1px solid var(--accent)' : '1px solid var(--border-subtle)'
            }"
            @click="handleSelectJob(job)"
          >
            <div style="display:flex; justify-content:space-between;">
              <strong style="font-size:12px;">{{ (job.request as any)?.title || 'Job #' + job.id.slice(0, 8) }}</strong>
              <span class="badge">{{ job.status }}</span>
            </div>
            <div style="font-size:11px; color:var(--text-muted); margin-top:2px;">Stage {{ job.next_stage_index }}/9 · rev {{ job.revision }}</div>
          </div>
        </div>
      </div>

      <!-- Right: Staged Progress & Stage Inspector -->
      <div style="flex:1; padding:24px 32px; overflow-y:auto;">
        <div v-if="!currentJob" style="padding:60px; text-align:center; color:var(--text-muted);">
          Select an active generation job on the left or create a new job.
        </div>

        <div v-else style="max-width:760px;">
          <!-- Stage Progress Stepper -->
          <div style="margin-bottom:24px;">
            <h3 style="font-size:13px; font-weight:700; margin-bottom:12px; color:var(--text-muted);">PIPELINE STAGES (9 PHASES)</h3>
            <div style="display:flex; gap:6px; flex-wrap:wrap;">
              <button
                v-for="stageRec in currentJob.stages"
                :key="stageRec.stage"
                :class="['btn', selectedStageId === stageRec.stage ? 'btn-primary' : 'btn-secondary']"
                style="font-size:11px; padding:4px 8px;"
                @click="selectStage(stageRec)"
              >
                ✓ {{ stageRec.stage }}
              </button>
            </div>
          </div>

          <!-- Stage Artifact Inspector / Editor -->
          <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px;">
            <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:10px;">
              <h3 style="font-size:13px; font-weight:700;">STAGE ARTIFACT: {{ selectedStageId }}</h3>
              <button class="btn btn-secondary" style="font-size:11px;" @click="handleSaveStageEdit">Save Stage Edit</button>
            </div>
            <p style="font-size:11px; color:var(--text-muted); margin-bottom:10px;">
              Editing a completed stage output invalidates all downstream stages and re-opens the pipeline.
            </p>
            <textarea
              v-model="stageEditText"
              class="form-textarea"
              style="min-height:280px; font-family:var(--font-mono); font-size:12px;"
            ></textarea>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
