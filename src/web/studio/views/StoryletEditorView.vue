<script setup lang="ts">
import { onMounted, ref, computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useStoryletEditorViewModel } from "../view-models/storyletEditor.ts";
import { studioState } from "../state.ts";

const route = useRoute();
const router = useRouter();
const storyletId = typeof route.params.id === "string" ? route.params.id : undefined;

const {
  activeTab,
  loading,
  isNew,
  draft,
  isApproved,
  loadStorylet,
  saveDraft,
  approveStorylet,
  addChoice,
  removeChoice,
  addRoleParameter,
  removeRoleParameter,
} = useStoryletEditorViewModel(storyletId);
const lastSavedSnapshot = ref<string>("");

onMounted(async () => {
  if (storyletId) {
    await loadStorylet(storyletId);
    lastSavedSnapshot.value = JSON.stringify(draft);
  } else {
    lastSavedSnapshot.value = JSON.stringify(draft);
  }
});

const isDirty = computed(() => {
  return JSON.stringify(draft) !== lastSavedSnapshot.value;
});

const canApprove = computed(() => {
  return !isDirty.value && !isApproved.value && !isNew.value;
});

function goBack() {
  if (isDirty.value) {
    if (!window.confirm("You have unsaved changes. Leave anyway?")) return;
  }
  router.push("/storylets");
}

async function handleSave() {
  const ok = await saveDraft();
  if (ok) {
    lastSavedSnapshot.value = JSON.stringify(draft);
    if (isNew.value) {
      router.replace(`/storylets/${draft.id}/edit`);
    }
  }
}

async function handleApprove() {
  const ok = await approveStorylet();
  if (ok) {
    lastSavedSnapshot.value = JSON.stringify(draft);
  }
}

// Precondition helpers
function addPrecondition(kind: "all_of" | "any_of") {
  if (!draft.preconditions) draft.preconditions = {};
  if (!draft.preconditions[kind]) draft.preconditions[kind] = [];
  draft.preconditions[kind]!.push({
    variable: studioState.qualities[0]?.name || "clue",
    op: "gte",
    value: 1,
  });
}

function removePrecondition(kind: "all_of" | "any_of", idx: number) {
  draft.preconditions[kind]?.splice(idx, 1);
}

// Choice helpers
function addChoiceCondition(choiceIdx: number) {
  if (!draft.choices[choiceIdx].conditions) draft.choices[choiceIdx].conditions = [];
  draft.choices[choiceIdx].conditions!.push({
    variable: studioState.qualities[0]?.name || "clue",
    op: "gte",
    value: 1,
  });
}

function removeChoiceCondition(choiceIdx: number, condIdx: number) {
  draft.choices[choiceIdx].conditions?.splice(condIdx, 1);
}

function updateChoiceEffect(choiceIdx: number, qKey: string, valStr: string) {
  if (!valStr.trim()) {
    delete draft.choices[choiceIdx].effects[qKey];
  } else {
    const num = Number(valStr);
    draft.choices[choiceIdx].effects[qKey] = isNaN(num) ? valStr : num;
  }
}

function updateStoryletEffect(qKey: string, valStr: string) {
  if (!valStr.trim()) {
    delete draft.effects[qKey];
  } else {
    const num = Number(valStr);
    draft.effects[qKey] = isNaN(num) ? valStr : num;
  }
}

// Client-side Unslop Audit Scan
const auditFindings = computed(() => {
  const text = (draft.content.sceneOpening || "") + " " + (draft.content.text_template || "");
  const findings: string[] = [];
  const lower = text.toLowerCase();

  const banned = ["delve", "tapestry", "landscape", "pivotal", "foster", "underscore", "testament", "crucial", "intricate", "robust", "harness", "quietly", "deeply", "bruised silence"];
  for (const w of banned) {
    if (lower.includes(w)) findings.push(`Banned buzzword detected: '${w}'`);
  }
  if (text.includes("—")) findings.push("Em-dash (—) pattern detected");
  if (/nots+[^.—\-]+[-—]s*it's/i.test(text)) findings.push("Negative parallelism detected ('not X - it's Y')");
  if (/nots+[^.—\-]+.s*nots+[^.—\-]+.s*justs+/i.test(text)) findings.push("Dramatic countdown pattern detected");

  return findings;
});
</script>

<template>
  <div class="full-editor">
    <header class="editor-toolbar">
      <div class="toolbar-left">
        <button class="btn btn-secondary" @click="goBack">
          ← Back to Storylets
        </button>
        <span style="font-size:14px; font-weight:600;">
          {{ draft.title || "Untitled Storylet" }}
        </span>
        <span :class="['badge', isApproved ? 'approved' : '']">
          {{ isApproved ? 'Approved' : 'Draft' }} · v{{ draft.revision }}
        </span>
        <span v-if="isDirty" class="badge" style="background:rgba(255,149,0,0.18); color:var(--status-warning);">
          Unsaved Edits
        </span>
      </div>

      <div class="toolbar-right">
        <div class="tab-nav">
          <button
            :class="['tab-btn', activeTab === 'content' ? 'active' : '']"
            @click="activeTab = 'content'"
          >
            Scene Content
          </button>
          <button
            :class="['tab-btn', activeTab === 'logic' ? 'active' : '']"
            @click="activeTab = 'logic'"
          >
            Preconditions & Choices
          </button>
          <button
            :class="['tab-btn', activeTab === 'review' ? 'active' : '']"
            @click="activeTab = 'review'"
          >
            Pacing & Style Audit
          </button>
        </div>

        <button class="btn btn-secondary" @click="handleSave">
          Save Draft
        </button>
        <button
          class="btn btn-primary"
          :disabled="!canApprove"
          :title="isDirty ? 'Save draft before approving' : 'Approve current revision'"
          @click="handleApprove"
        >
          Approve Revision
        </button>
      </div>
    </header>

    <div class="editor-body">
      <!-- CONTENT TAB -->
      <div v-if="activeTab === 'content'">
        <div class="form-group">
          <label class="form-label">Scene Title</label>
          <input v-model="draft.title" class="form-input" placeholder="e.g. A Damp Ticket at the Counter" />
        </div>

        <div style="display:grid; grid-template-columns:1fr 1fr; gap:16px;">
          <div class="form-group">
            <label class="form-label">Location Constraint</label>
            <select v-model="draft.location" class="form-select">
              <option :value="null">Any Location</option>
              <option v-for="l in studioState.locations" :key="l.id" :value="l.id">
                {{ l.name }}
              </option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">Group Tag</label>
            <input v-model="draft.group_tag" class="form-input" placeholder="e.g. act1, investigation" />
          </div>
        </div>

        <div class="form-group">
          <label class="form-label">Scene Opening / Prose</label>
          <textarea
            v-model="draft.content.sceneOpening"
            class="form-textarea"
            style="min-height:160px; font-family:var(--font-serif);"
            placeholder="Describe the moment the player arrives or the conflict unfolds..."
          ></textarea>
        </div>

        <div class="form-group">
          <label class="form-label">Text Template (Fallback)</label>
          <textarea
            v-model="draft.content.text_template"
            class="form-textarea"
            style="min-height:80px;"
            placeholder="Fallback scene text..."
          ></textarea>
        </div>
      </div>

      <!-- LOGIC & CHOICES TAB -->
      <div v-else-if="activeTab === 'logic'">
        <!-- Preconditions -->
        <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:20px;">
          <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
            <h3 style="font-size:13px; font-weight:700;">PRECONDITION PREDICATES (ALL OF)</h3>
            <button class="btn btn-secondary" style="font-size:11px;" @click="addPrecondition('all_of')">+ Add Condition</button>
          </div>

          <div v-if="!draft.preconditions.all_of?.length" style="font-size:12px; color:var(--text-muted);">
            No preconditions required. This storylet can trigger whenever roles match.
          </div>

          <div v-for="(pred, pIdx) in draft.preconditions.all_of ?? []" :key="pIdx" style="display:flex; gap:10px; margin-bottom:8px; align-items:center;">
            <select v-model="pred.variable" class="form-select" style="flex:1;">
              <option value="location">Current Location</option>
              <option v-for="q in studioState.qualities" :key="q.name" :value="q.name">{{ q.name }}</option>
            </select>
            <select v-model="pred.op" class="form-select" style="width:110px;">
              <option value="eq">== (Equal)</option>
              <option value="neq">!= (Not Equal)</option>
              <option value="gt">> (Greater)</option>
              <option value="gte">>= (At Least)</option>
              <option value="lt">< (Less)</option>
              <option value="lte"><= (At Most)</option>
            </select>
            <input v-model="pred.value" class="form-input" style="width:120px;" placeholder="Value" />
            <button class="btn" style="color:var(--status-danger);" @click="removePrecondition('all_of', pIdx)">✕</button>
          </div>
        </div>

        <!-- Role Parameters -->
        <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px; margin-bottom:20px;">
          <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:12px;">
            <h3 style="font-size:13px; font-weight:700;">REQUIRED ACTOR ROLES</h3>
            <button class="btn btn-secondary" style="font-size:11px;" @click="addRoleParameter">+ Add Role</button>
          </div>

          <div v-for="(param, idx) in draft.parameters" :key="idx" style="display:flex; gap:10px; margin-bottom:8px;">
            <input v-model="param.param_name" class="form-input" placeholder="Param name (e.g. witness)" style="width:180px;" />
            <input v-model="param.query" class="form-input" placeholder='role == "witness"' style="flex:1;" />
            <button class="btn" style="color:var(--status-danger);" @click="removeRoleParameter(idx)">✕</button>
          </div>
        </div>

        <!-- Choices with Effects & Conditions -->
        <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px;">
          <div style="display:flex; justify-content:space-between; align-items:center; margin-bottom:14px;">
            <h3 style="font-size:13px; font-weight:700;">PLAYER CHOICES</h3>
            <button class="btn btn-secondary" style="font-size:11px;" @click="addChoice">+ Add Choice</button>
          </div>

          <div v-for="(choice, cIdx) in draft.choices" :key="choice.choice_id" style="padding:14px; background:var(--bg-input); border:1px solid var(--border-subtle); border-radius:10px; margin-bottom:12px;">
            <div style="display:flex; gap:10px; margin-bottom:8px; align-items:center;">
              <span style="font-weight:700; font-size:12px;">Choice #{{ cIdx + 1 }}</span>
              <input v-model="choice.label" class="form-input" placeholder="Choice label text" style="flex:1;" />
              <button class="btn" style="color:var(--status-danger);" @click="removeChoice(cIdx)">✕</button>
            </div>

            <div style="display:grid; grid-template-columns:1fr 1fr; gap:10px; margin-bottom:10px;">
              <div>
                <label class="form-label">System Verb</label>
                <input v-model="choice.systemVerb" class="form-input" placeholder="e.g. investigate, flee" />
              </div>
              <div>
                <label class="form-label">Divert Target Storylet ID (Optional)</label>
                <input v-model="choice.divert_target" class="form-input" placeholder="e.g. stl_aftermath" />
              </div>
            </div>

            <!-- Choice Effects -->
            <div>
              <label class="form-label">Choice Quality Effects (Delta or Set Value)</label>
              <div style="display:flex; flex-wrap:wrap; gap:8px;">
                <div v-for="q in studioState.qualities" :key="q.name" style="padding:4px 8px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:6px; font-size:11px;">
                  <span>{{ q.name }}:</span>
                  <input
                    :value="choice.effects[q.name] ?? ''"
                    style="width:50px; margin-left:4px; padding:2px 4px; border:1px solid var(--border-subtle); border-radius:4px;"
                    placeholder="—"
                    @input="updateChoiceEffect(cIdx, q.name, ($event.target as HTMLInputElement).value)"
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- REVIEW & PACING TAB -->
      <div v-else-if="activeTab === 'review'">
        <div style="display:grid; grid-template-columns:1fr 1fr; gap:16px; margin-bottom:16px;">
          <div class="form-group">
            <label class="form-label">Cluster Type</label>
            <select v-model="draft.cluster_type" class="form-select">
              <option value="player_controlled_passage">Player-Controlled Passage (Hub)</option>
              <option value="linear">Linear Progression (Climax)</option>
              <option value="wheel">Wheel / Deck (Randomized)</option>
              <option value="aftermath">Aftermath / Accelerator</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">Repeatability</label>
            <select v-model="draft.repeatability" class="form-select">
              <option value="always">Always</option>
              <option value="once_ever">Once Ever</option>
              <option value="cooldown">Cooldown Turns</option>
              <option value="persistent">Persistent</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">Urgency Weight (0 - 100)</label>
            <input v-model.number="draft.urgency_weight" type="number" class="form-input" min="0" max="100" />
          </div>

          <div class="form-group">
            <label class="form-label">Cooldown Turns</label>
            <input v-model.number="draft.cooldown_turns" type="number" class="form-input" min="0" />
          </div>
        </div>

        <div class="form-group" style="margin-bottom:20px;">
          <label style="display:flex; align-items:center; gap:8px; font-weight:600; cursor:pointer;">
            <input v-model="draft.ending" type="checkbox" style="width:16px; height:16px;" />
            <span>Designated Terminal Ending (Playtest terminates when reached)</span>
          </label>
        </div>

        <!-- Live Prose Audit Scan -->
        <div style="padding:16px; background:var(--bg-card); border:1px solid var(--border-subtle); border-radius:12px;">
          <h3 style="font-size:13px; font-weight:700; margin-bottom:8px;">UNSLOP PROSE AUDIT SCAN</h3>
          <div v-if="auditFindings.length === 0" style="color:var(--status-success); font-size:12px;">
            ✓ Clean prose: No banned buzzwords, em-dash clauses, or dramatic countdowns flagged.
          </div>
          <div v-else>
            <div style="color:var(--status-warning); font-size:12px; margin-bottom:6px;">
              {{ auditFindings.length }} Style Finding{{ auditFindings.length === 1 ? '' : 's' }}:
            </div>
            <ul style="margin-left:18px; font-size:12px; color:var(--text-muted);">
              <li v-for="(f, i) in auditFindings" :key="i">{{ f }}</li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>
