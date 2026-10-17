/**
 * Storylet Studio — Narrative Designer Controller
 * Drives the Left Navigation Rail, Tri-Pane Workshop, Master-Detail Codex,
 * Aristotelian Drama Director, and Butterfly Causal Timeline.
 */

import { GENERATION_STAGES, type GenerationJob } from "../agents/GenerationWorkflow.ts";
import { summarizeUnslopViolations } from "../agents/UnslopAudit.ts";

export class DesignerStudioController {
  private lastGeneratedStorylet: any = null;
  private cachedQualities: any[] = [];
  private cachedCharacters: any[] = [];
  private cachedLocations: any[] = [];
  private cachedStorylets: any[] = [];
  private playtestSessionId: string | null = null;
  private playtestEvents: EventSource | null = null;
  private playtestEventsSessionId: string | null = null;
  private playtestSnapshot: any = null;
  private playtestDecisionOverride: any = null;
  private selectedButterflyEventId: string | null = null;
  private playtestBranches: Array<{ sessionId: string; label: string }> = [];
  private cachedScenarios: any[] = [];
  private pendingProjectBundle: unknown = null;
  private projectMetadata: { id: string; name: string; revision: number } | null = null;
  private savedManuscriptSnapshot: string | null = null;
  private generationJob: GenerationJob | null = null;
  private selectedGenerationStage: string | null = null;
  private generationEvents: EventSource | null = null;
  private lastGenerationAudit: { passed: boolean; violations: string[] } | null = null;
  private currentCodexFilter: string = "all";
  private currentCodexSearch: string = "";

  constructor() {
    this.bindNavigation();
    this.bindCodexMasterDetail();
    this.bindEntityActions();
    this.bindWorkshopActions();
    this.bindDirectorActions();
    this.bindButterflyActions();
    this.bindPlaytestActions();
    this.bindProjectBundleActions();
  }

  public async init(): Promise<void> {
    await this.loadProjectMetadata();
    await this.refreshCodexAll();
    this.loadStoryletDeck();
    this.refreshDirectorEvaluation();
    this.refreshButterflyChain();
    await this.loadLatestGenerationJob();
    await this.loadSavedPlaytests();
    await this.loadPlaytestScenarios();
  }

  private bindProjectBundleActions(): void {
    document.getElementById("btn-save-project-name")?.addEventListener("click", () => this.saveProjectName());
    document.getElementById("btn-export-project")?.addEventListener("click", () => this.exportProjectBundle());
    const fileInput = document.getElementById("project-bundle-file") as HTMLInputElement | null;
    document.getElementById("btn-select-project-bundle")?.addEventListener("click", () => fileInput?.click());
    fileInput?.addEventListener("change", () => this.reviewProjectBundle());
    document.getElementById("btn-import-project")?.addEventListener("click", () => this.importProjectBundle());
  }

  private async loadProjectMetadata(): Promise<void> {
    try {
      const response = await fetch("/api/v1/project");
      const metadata = await response.json();
      if (!response.ok) throw new Error(metadata.error ?? "Could not load project settings.");
      this.projectMetadata = metadata;
      const nameInput = document.getElementById("project-name-input") as HTMLInputElement | null;
      if (nameInput) nameInput.value = metadata.name;
    } catch (error) {
      this.setProjectBundleStatus(error instanceof Error ? error.message : "Could not load project settings.");
    }
  }

  private setProjectBundleStatus(message: string): void {
    const status = document.getElementById("project-bundle-status");
    if (status) status.textContent = message;
  }

  private async saveProjectName(): Promise<void> {
    const nameInput = document.getElementById("project-name-input") as HTMLInputElement | null;
    const name = nameInput?.value.trim();
    if (!name) {
      this.setProjectBundleStatus("Give this project a name before saving.");
      nameInput?.focus();
      return;
    }
    try {
      const response = await fetch("/api/v1/project", {
        method: "PATCH", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ name }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not save the project name.");
      this.projectMetadata = body;
      this.setProjectBundleStatus(`Saved project name · revision ${body.revision}`);
    } catch (error) {
      this.setProjectBundleStatus(error instanceof Error ? error.message : "Could not save the project name.");
    }
  }

  private exportProjectBundle(): void {
    try {
      const name = this.projectMetadata?.name ?? "storylet-project";
      const anchor = document.createElement("a");
      anchor.style.display = "none";
      anchor.href = "/api/v1/project-bundle";
      document.body.append(anchor);
      anchor.click();
      anchor.remove();
      this.setProjectBundleStatus(`Exported ${name}.`);
    } catch (error) {
      this.setProjectBundleStatus(error instanceof Error ? error.message : "Could not export this project.");
    }
  }

  private async reviewProjectBundle(): Promise<void> {
    const fileInput = document.getElementById("project-bundle-file") as HTMLInputElement | null;
    const importButton = document.getElementById("btn-import-project") as HTMLButtonElement | null;
    const file = fileInput?.files?.[0];
    this.pendingProjectBundle = null;
    if (importButton) importButton.disabled = true;
    if (!file) return;
    try {
      const bundle = JSON.parse(await file.text());
      const response = await fetch("/api/v1/project-bundle/validate", {
        method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ bundle }),
      });
      const result = await response.json();
      if (!response.ok || !result.valid) {
        throw new Error((result.issues ?? [result.error ?? "This project file could not be validated."]).join(" · "));
      }
      this.pendingProjectBundle = bundle;
      if (importButton) importButton.disabled = false;
      const counts = Object.entries(result.counts as Record<string, number>).map(([key, value]) => `${value} ${key}`).join(" · ");
      this.setProjectBundleStatus(`Ready to import “${result.project.name}” · ${counts}. Your current project remains unchanged until you replace it.`);
    } catch (error) {
      this.setProjectBundleStatus(error instanceof Error ? error.message : "Choose a valid project JSON file.");
    }
  }

  private async importProjectBundle(): Promise<void> {
    if (!this.pendingProjectBundle) return;
    const incoming = this.pendingProjectBundle as { project?: { name?: string } };
    const currentName = this.projectMetadata?.name ?? "this project";
    if (!window.confirm(`Replace “${currentName}” with “${incoming.project?.name ?? "the selected project"}”? This replaces authored content, scenarios, and generation drafts. Saved playtest runs remain available.`)) return;
    const button = document.getElementById("btn-import-project") as HTMLButtonElement | null;
    if (button) button.disabled = true;
    try {
      const response = await fetch("/api/v1/project-bundle/import", {
        method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ bundle: this.pendingProjectBundle, confirmReplace: true }),
      });
      const result = await response.json();
      if (!response.ok) throw new Error([result.error, ...(result.issues ?? [])].filter(Boolean).join(" · "));
      this.pendingProjectBundle = null;
      (document.getElementById("project-bundle-file") as HTMLInputElement).value = "";
      this.projectMetadata = result.project;
      (document.getElementById("project-name-input") as HTMLInputElement).value = result.project.name;
      this.setProjectBundleStatus(`Imported “${result.project.name}”. ${Object.entries(result.counts as Record<string, number>).map(([key, value]) => `${value} ${key}`).join(" · ")}`);
      await Promise.all([this.refreshCodexAll(), this.loadPlaytestLocations(), this.loadStoryletDeck(), this.loadSavedPlaytests()]);
    } catch (error) {
      this.setProjectBundleStatus(error instanceof Error ? error.message : "Could not import this project.");
      if (button) button.disabled = false;
    }
  }

  // --- Left Navigation Rail ---
  private bindNavigation(): void {
    const navButtons = document.querySelectorAll(".nav-item-btn");
    navButtons.forEach((btn) => {
      btn.addEventListener("click", (e) => {
        const targetId = (e.currentTarget as HTMLElement).getAttribute("data-nav");
        navButtons.forEach((b) => b.classList.remove("active"));
        (e.currentTarget as HTMLElement).classList.add("active");

        document.querySelectorAll(".view-pane").forEach((pane) => pane.classList.remove("active"));
        if (targetId) {
          document.getElementById(targetId)?.classList.add("active");

          // Auto-refresh when entering view
          if (targetId === "view-director") this.refreshDirectorEvaluation();
          if (targetId === "view-butterfly") this.refreshButterflyChain();
          if (targetId === "view-codex") this.refreshCodexAll();
          if (targetId === "view-workshop") this.loadStoryletDeck();
          if (targetId === "view-playtest") {
            window.dispatchEvent(new Event("resize"));
            this.loadPlaytestLocations();
            this.loadSavedPlaytests();
          }
        }
      });
    });
  }

  // --- Codex Master-Detail Navigation ---
  private bindCodexMasterDetail(): void {
    // Legacy Tab Buttons (Backward Compatibility)
    const codexTabs = document.querySelectorAll(".codex-tab-btn");
    codexTabs.forEach((tab) => {
      tab.addEventListener("click", (e) => {
        const targetSection = (e.currentTarget as HTMLElement).getAttribute("data-codex");
        codexTabs.forEach((t) => t.classList.remove("active"));
        (e.currentTarget as HTMLElement).classList.add("active");

        document.querySelectorAll(".codex-view-section").forEach((sec) => sec.classList.add("hidden"));
        if (targetSection) {
          document.getElementById(targetSection)?.classList.remove("hidden");
        }
      });
    });

    // Master Sidebar Category Chips
    const chips = document.querySelectorAll(".codex-chip-btn");
    chips.forEach((chip) => {
      chip.addEventListener("click", (e) => {
        chips.forEach((c) => c.classList.remove("active"));
        (e.currentTarget as HTMLElement).classList.add("active");
        this.currentCodexFilter = (e.currentTarget as HTMLElement).getAttribute("data-filter") || "all";
        this.renderCodexMasterList();

        // Also switch right-hand form pane accordingly
        if (this.currentCodexFilter === "currency") this.switchCodexSection("codex-qualities");
        else if (this.currentCodexFilter === "character") this.switchCodexSection("codex-characters");
        else if (this.currentCodexFilter === "location") this.switchCodexSection("codex-locations");
      });
    });

    // Search Input
    const searchInput = document.getElementById("codex-search-input") as HTMLInputElement;
    if (searchInput) {
      searchInput.addEventListener("input", (e) => {
        this.currentCodexSearch = (e.target as HTMLInputElement).value.toLowerCase().trim();
        this.renderCodexMasterList();
      });
    }
  }

  private switchCodexSection(sectionId: string): void {
    document.querySelectorAll(".codex-view-section").forEach((sec) => sec.classList.add("hidden"));
    document.getElementById(sectionId)?.classList.remove("hidden");

    // Sync legacy tab button active state
    document.querySelectorAll(".codex-tab-btn").forEach((t) => {
      if (t.getAttribute("data-codex") === sectionId) t.classList.add("active");
      else t.classList.remove("active");
    });
  }
  private focusCodexCard(cardId: string): void {
    setTimeout(() => {
      document.querySelectorAll(".codex-card").forEach((c) => c.classList.remove("highlighted"));
      const card = document.getElementById(cardId);
      if (card) {
        card.classList.add("highlighted");
        card.scrollIntoView({ behavior: "smooth", block: "center" });
      }
    }, 50);
  }

  // --- Entity Actions ---
  private bindEntityActions(): void {
    document.getElementById("btn-add-quality")?.addEventListener("click", () => this.onAddQuality());
    document.getElementById("btn-add-character")?.addEventListener("click", () => this.onAddCharacter());
    document.getElementById("btn-add-npc-rule")?.addEventListener("click", () => this.onAddNpcBehavior());
    document.getElementById("btn-add-location")?.addEventListener("click", () => this.onAddLocation());
  }

  // --- Workshop Actions ---
  private bindWorkshopActions(): void {
    document.getElementById("btn-run-strata-gen")?.addEventListener("click", () => this.onGenerateStorylet());
    document.getElementById("btn-next-generation-stage")?.addEventListener("click", () => this.onAdvanceGenerationStage());
    document.getElementById("btn-save-generation-stage")?.addEventListener("click", () => this.onSaveGenerationStage());
    document.getElementById("btn-save-generation-draft")?.addEventListener("click", () => this.onSaveGenerationDraft());
    document.getElementById("btn-save-generated-storylet")?.addEventListener("click", () => this.onSaveGeneratedStorylet());
    document.getElementById("btn-approve-storylet")?.addEventListener("click", () => this.onApproveGeneratedStorylet());
    document.getElementById("generation-stage-list")?.addEventListener("click", (event) => {
      const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-stage]");
      if (!button) return;
      this.selectedGenerationStage = button.dataset.stage ?? null;
      if (this.generationJob) this.renderGenerationJob(this.generationJob);
    });
  }

  // --- Director Actions ---
  private bindDirectorActions(): void {
    document.getElementById("btn-director-play-selected")?.addEventListener("click", () => this.onPlayDirectorSelection());
    document.getElementById("btn-director-rollback")?.addEventListener("click", () => this.onOpenBranchTools());
  }

  // --- Butterfly Actions ---
  private bindButterflyActions(): void {
    document.getElementById("btn-run-butterfly-tick")?.addEventListener("click", () => this.onRunButterflyTick());
    window.addEventListener("resize", () => this.refreshButterflyChain());
    document.getElementById("butterfly-canvas")?.addEventListener("click", (event) => {
      const canvas = event.currentTarget as HTMLCanvasElement;
      const rect = canvas.getBoundingClientRect();
      const events = this.playtestSnapshot?.world?.events ?? [];
      const points = this.butterflyPoints(events, rect.width, rect.height);
      const x = (event as MouseEvent).clientX - rect.left;
      const y = (event as MouseEvent).clientY - rect.top;
      const selected = points.find((point) => Math.hypot(point.x - x, point.y - y) < 18);
      const detail = document.getElementById("butterfly-event-detail");
      if (detail && selected) {
        const item = events.find((entry: any) => entry.id === selected.id);
        if (item) {
          this.selectedButterflyEventId = item.id;
          detail.textContent = `Turn ${item.turn} · ${item.actor_id} · ${item.action}. ${item.deltas?.length ? item.deltas.map((delta: any) => `${delta.key}: ${JSON.stringify(delta.before)} → ${JSON.stringify(delta.after)}`).join("; ") : "No quality changes."}${item.caused_by?.length ? ` Caused by ${item.caused_by.join(", ")}.` : " Root event."}`;
          this.renderBranchControls();
          this.refreshButterflyChain();
        }
      }
    });
  }

  private bindPlaytestActions(): void {
    document.getElementById("btn-start-clean-playtest")?.addEventListener("click", () => this.startCleanPlaytest());
    document.getElementById("btn-save-playtest-scenario")?.addEventListener("click", () => this.savePlaytestScenario());
    document.getElementById("btn-new-playtest-scenario")?.addEventListener("click", () => this.newPlaytestScenario());
    document.getElementById("btn-delete-playtest-scenario")?.addEventListener("click", () => this.deletePlaytestScenario());
    document.getElementById("playtest-scenario-select")?.addEventListener("change", () => {
      const selected = this.cachedScenarios.find((scenario) => scenario.id === (document.getElementById("playtest-scenario-select") as HTMLSelectElement).value);
      if (selected) this.populateScenarioEditor(selected);
      else this.newPlaytestScenario();
    });
    this.bindScenarioPolicyControls();
    document.getElementById("btn-resume-playtest")?.addEventListener("click", () => this.resumeSavedPlaytest());
    document.getElementById("btn-refresh-playtest")?.addEventListener("click", () => this.refreshPlaytest());
    document.getElementById("btn-rank-local-encoder")?.addEventListener("click", () => this.rankPlaytestWithLocalEncoder());
    document.getElementById("btn-playtest-wait")?.addEventListener("click", () => this.submitPlaytestAction("wait", []));
    document.getElementById("btn-playtest-fork-event")?.addEventListener("click", () => this.forkPlaytestAtSelectedEvent());
    document.getElementById("btn-playtest-compare")?.addEventListener("click", () => this.comparePlaytestBranches());
    document.getElementById("playtest-compare-run")?.addEventListener("change", () => this.renderBranchControls());
    document.getElementById("playtest-location-actions")?.addEventListener("click", (event) => {
      const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-location-id]");
      if (!button) return;
      this.submitPlaytestAction("move", [{ kind: "move_player", location_id: button.dataset.locationId }]);
    });
    document.getElementById("playtest-choice-list")?.addEventListener("click", (event) => {
      const button = (event.target as HTMLElement).closest<HTMLButtonElement>("button[data-storylet-id][data-choice-id]");
      if (!button) return;
      this.submitPlaytestChoice(button.dataset.storyletId!, button.dataset.choiceId!, button);
    });
  }

  // =========================================================================
  // CODEX LOADERS & RENDERING
  // =========================================================================

  private async refreshCodexAll(): Promise<void> {
    await Promise.all([this.loadQualities(), this.loadCharacters(), this.loadLocations()]);
    await this.loadNpcBehaviors();
    this.renderCodexMasterList();
  }

  private renderCodexMasterList(): void {
    const listContainer = document.getElementById("codex-master-list");
    if (!listContainer) return;
    listContainer.innerHTML = "";

    // Combine all entries
    const items: Array<{ id: string; name: string; type: string; category: string; desc: string }> = [];

    // Qualities
    for (const q of this.cachedQualities) {
      items.push({
        id: q.name,
        name: q.name,
        type: q.type,
        category: "currency",
        desc: `Value: ${q.value}`
      });
    }

    // Characters (deduplicate by normalized name or ID)
    const seenCharNames = new Set<string>();
    for (const c of this.cachedCharacters) {
      const normName = c.name.toLowerCase().trim();
      if (seenCharNames.has(normName)) continue;
      seenCharNames.add(normName);
      items.push({
        id: c.id,
        name: c.name,
        type: "character",
        category: "character",
        desc: `${c.role} • ${c.home_location ?? c.homeLocation ?? "District"}`
      });
    }

    // Locations (deduplicate by ID)
    const seenLocIds = new Set<string>();
    for (const l of this.cachedLocations) {
      if (seenLocIds.has(l.id)) continue;
      seenLocIds.add(l.id);
      items.push({
        id: l.id,
        name: l.name,
        type: "location",
        category: "location",
        desc: l.id
      });
    }

    // Apply Filter & Search
    const filtered = items.filter((item) => {
      const matchFilter = this.currentCodexFilter === "all" || item.category === this.currentCodexFilter;
      const matchSearch =
        !this.currentCodexSearch ||
        item.name.toLowerCase().includes(this.currentCodexSearch) ||
        item.desc.toLowerCase().includes(this.currentCodexSearch);
      return matchFilter && matchSearch;
    });

    for (const item of filtered) {
      const row = document.createElement("div");
      row.className = "codex-entry-item";
      row.innerHTML = `
        <div>
          <div class="entry-item-title">${item.name}</div>
          <div class="entry-item-meta">${item.desc}</div>
        </div>
        <span class="codex-tag tag-${item.type}">${item.type}</span>
      `;

      row.addEventListener("click", () => {
        document.querySelectorAll(".codex-entry-item").forEach((el) => el.classList.remove("active"));
        row.classList.add("active");

        if (item.category === "currency") {
          this.switchCodexSection("codex-qualities");
          this.focusCodexCard(`codex-card-quality-${item.id}`);
        } else if (item.category === "character") {
          this.switchCodexSection("codex-characters");
          this.focusCodexCard(`codex-card-char-${item.id}`);
        } else if (item.category === "location") {
          this.switchCodexSection("codex-locations");
          this.focusCodexCard(`codex-card-loc-${item.id}`);
        }
      });
      listContainer.appendChild(row);
    }
  }

  private async loadQualities(): Promise<void> {
    try {
      const res = await fetch("/api/qualities");
      const list = await res.json();
      this.cachedQualities = list;
      const container = document.getElementById("qualities-editor-list")!;
      container.innerHTML = "";

      for (const q of list) {
        const card = document.createElement("div");
        card.className = "codex-card";
        card.id = `codex-card-quality-${q.name}`;
        card.innerHTML = `
          <div class="codex-card-header">
            <span class="codex-card-title">${q.name}</span>
            <span class="codex-tag tag-${q.type}">${q.type}</span>
          </div>
          <div class="codex-card-desc" style="font-size: 11.5px; color: var(--text-muted); margin-bottom: 8px;">
            ${q.description ?? "Tracked quality metric."}
          </div>
          <div style="display: flex; justify-content: space-between; align-items: center; border-top: 1px solid var(--border-subtle); padding-top: 8px;">
            <span style="font-size: 11px; color: var(--text-muted);">Current Value:</span>
            <input type="number" class="form-input" style="width: 80px; text-align: right; padding: 4px 8px;" value="${q.value}"
              data-qname="${q.name}" data-qtype="${q.type}">
          </div>
        `;

        card.querySelector("input")?.addEventListener("change", async (e) => {
          const input = e.target as HTMLInputElement;
          await fetch("/api/qualities", {
            method: "POST",
            headers: { "Content-Type": "application/json" },
            body: JSON.stringify({
              name: input.getAttribute("data-qname"),
              type: input.getAttribute("data-qtype"),
              value: Number(input.value)
            })
          });
        });

        container.appendChild(card);
      }
    } catch (e) {
      console.error("Failed to load qualities:", e);
    }
  }

  private async onAddQuality(): Promise<void> {
    const nameInput = document.getElementById("new-quality-name") as HTMLInputElement;
    const typeSelect = document.getElementById("new-quality-type") as HTMLSelectElement;
    const valInput = document.getElementById("new-quality-val") as HTMLInputElement;

    const name = nameInput.value.trim();
    if (!name) return;

    await fetch("/api/qualities", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        name,
        type: typeSelect.value,
        value: Number(valInput.value)
      })
    });

    nameInput.value = "";
    await this.loadQualities();
    await this.loadNpcBehaviors();
    this.renderCodexMasterList();
  }

  private async loadCharacters(): Promise<void> {
    try {
      const res = await fetch("/api/characters");
      const list = await res.json();
      this.cachedCharacters = list;
      const container = document.getElementById("characters-editor-list")!;
      container.innerHTML = "";

      for (const c of list) {
        const card = document.createElement("div");
        card.className = "codex-card";
        card.id = `codex-card-char-${c.id}`;
        card.innerHTML = `
          <div class="codex-card-header">
            <div style="display: flex; align-items: center; gap: 8px;">
              <span class="character-avatar-badge" style="width: 28px; height: 28px; font-size: 11px;">${c.name.substring(0, 2).toUpperCase()}</span>
              <div>
                <span class="codex-card-title">${c.name}</span>
                <span class="shift-pill" style="margin-left: 6px; font-size: 10px;">${c.role}</span>
              </div>
            </div>
            <button class="btn-hud-action" style="padding: 2px 7px; color: var(--accent-rose);" data-del-char="${c.id}" title="Delete character">✕</button>
          </div>
          <div class="codex-card-meta-grid">
            <div><span class="codex-meta-label">Home location:</span> <span class="codex-meta-val">${c.home_location ?? c.homeLocation ?? "Unassigned"}</span></div>
            <div style="grid-column: span 2;"><span class="codex-meta-label">Status:</span> <span class="codex-meta-val">${Object.entries(c.status_axes ?? c.status ?? {}).map(([key, value]) => `${key}: ${String(value)}`).join(" · ") || "No status fields"}</span></div>
          </div>
          <div class="codex-card-desc" style="font-size: 12px; line-height: 1.5; margin: 4px 0 8px 0;">
            <div><strong>Agenda:</strong> <span style="color: #cbd5e1;">${c.agenda}</span></div>
            <div style="color: var(--accent-rose); margin-top: 4px;"><strong>Red Line:</strong> <span>${c.red_line ?? c.redLine}</span></div>
          </div>
        `;
        card.querySelector("button")?.addEventListener("click", async (e) => {
          const id = (e.target as HTMLElement).getAttribute("data-del-char");
          if (id) {
            await fetch(`/api/characters/${id}`, { method: "DELETE" });
            await this.loadCharacters();
            await this.loadNpcBehaviors();
            this.renderCodexMasterList();
          }
        });

        container.appendChild(card);
      }
    } catch (e) {
      console.error("Failed to load characters:", e);
    }
  }

  private async onAddCharacter(): Promise<void> {
    const idInput = document.getElementById("new-char-id") as HTMLInputElement;
    const nameInput = document.getElementById("new-char-name") as HTMLInputElement;
    const roleInput = document.getElementById("new-char-role") as HTMLInputElement;
    const locSelect = document.getElementById("new-char-location") as HTMLSelectElement;
    const agendaInput = document.getElementById("new-char-agenda") as HTMLInputElement;
    const redlineInput = document.getElementById("new-char-redline") as HTMLInputElement;

    const id = idInput.value.trim();
    const name = nameInput.value.trim();
    const role = roleInput.value.trim();
    const location = locSelect.value;
    const agenda = agendaInput.value.trim();
    const redLine = redlineInput.value.trim();

    if (!id || !name || !role) {
      alert("Please provide Character ID, Name, and Role.");
      return;
    }

    await fetch("/api/characters", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        id,
        name,
        role,
        home_location: location,
        current_location: location,
        status_axes: {},
        needs: {},
        agenda,
        red_line: redLine
      })
    });

    idInput.value = "";
    nameInput.value = "";
    roleInput.value = "";
    agendaInput.value = "";
    redlineInput.value = "";

    await this.loadCharacters();
    await this.loadNpcBehaviors();
    this.renderCodexMasterList();
  }

  private async loadNpcBehaviors(): Promise<void> {
    const actorSelect = document.getElementById("npc-rule-actor") as HTMLSelectElement | null;
    const qualitySelects = ["npc-rule-quality", "npc-rule-effect-quality"]
      .map((id) => document.getElementById(id) as HTMLSelectElement | null);
    if (actorSelect) {
      actorSelect.replaceChildren(new Option(this.cachedCharacters.length ? "Choose a character" : "Create a character first", ""));
      for (const character of this.cachedCharacters) actorSelect.add(new Option(`${character.name} · ${character.role}`, character.id));
    }
    for (const select of qualitySelects) {
      if (!select) continue;
      select.replaceChildren(new Option(this.cachedQualities.length ? "Choose a quality" : "Create a quality first", ""));
      for (const quality of this.cachedQualities) select.add(new Option(quality.name, quality.name));
    }
    try {
      const response = await fetch("/api/npc-behaviors");
      if (!response.ok) throw new Error("Could not load NPC behavior rules.");
      const rules = await response.json();
      const list = document.getElementById("npc-behavior-list");
      if (!list) return;
      list.replaceChildren();
      for (const rule of rules) {
        const actor = this.cachedCharacters.find((character) => character.id === rule.actor_id);
        const card = document.createElement("article");
        card.className = "codex-card";
        const heading = document.createElement("div");
        heading.className = "codex-card-header";
        const title = document.createElement("strong");
        title.className = "codex-card-title";
        title.textContent = `${actor?.name ?? rule.actor_id}: ${rule.action}`;
        const remove = document.createElement("button");
        remove.type = "button";
        remove.className = "btn-hud-action";
        remove.textContent = "Remove";
        remove.addEventListener("click", async () => {
          const result = await fetch(`/api/npc-behaviors/${encodeURIComponent(rule.id)}`, { method: "DELETE" });
          if (result.ok) await this.loadNpcBehaviors();
        });
        heading.append(title, remove);
        const body = document.createElement("p");
        body.className = "playtest-help";
        const predicates = rule.conditions?.all_of ?? [];
        const effects = (rule.effects ?? []).map((effect: any) => `${effect.kind} ${effect.key ?? effect.to}: ${effect.amount ?? JSON.stringify(effect.value)}`);
        body.textContent = `Priority ${rule.priority} · when ${predicates.map((item: any) => `${item.variable} ${item.op} ${JSON.stringify(item.value)}`).join(" and ") || "always"} · ${effects.join(", ") || "no effects"}`;
        card.append(heading, body);
        list.appendChild(card);
      }
      if (!rules.length) {
        const empty = document.createElement("p");
        empty.className = "playtest-list-empty";
        empty.textContent = "No autonomous behavior rules. Add a rule to let characters react to world changes.";
        list.appendChild(empty);
      }
    } catch (error) {
      console.error("Could not load NPC behavior rules:", error);
    }
  }

  private async onAddNpcBehavior(): Promise<void> {
    const value = (id: string) => (document.getElementById(id) as HTMLInputElement | HTMLSelectElement).value.trim();
    const id = value("npc-rule-id");
    const actorId = value("npc-rule-actor");
    const action = value("npc-rule-action");
    const priority = Number(value("npc-rule-priority"));
    const conditionKey = value("npc-rule-quality");
    const effectKey = value("npc-rule-effect-quality");
    const effectKind = value("npc-rule-effect-kind");
    const conditionText = value("npc-rule-condition-value");
    const effectText = value("npc-rule-effect-value");
    if (!id || !actorId || !action || !Number.isInteger(priority) || !effectKey || !effectText) {
      alert("Provide a rule ID, character, action, integer priority, and effect quality and value.");
      return;
    }
    let effectValue: unknown;
    let conditionValue: unknown;
    try {
      effectValue = effectKind === "add_quality" ? Number(effectText) : JSON.parse(effectText);
      if (effectKind === "add_quality" && !Number.isFinite(effectValue)) throw new Error("Effect amount must be numeric.");
      if (conditionKey && conditionText) conditionValue = JSON.parse(conditionText);
    } catch (error) {
      alert(error instanceof Error ? error.message : "Condition and set values must be valid JSON.");
      return;
    }
    const rule = {
      id,
      actor_id: actorId,
      priority,
      action,
      conditions: conditionKey && conditionText ? {
        all_of: [{ variable: conditionKey, op: value("npc-rule-operator"), value: conditionValue }],
      } : {},
      effects: [effectKind === "add_quality"
        ? { kind: effectKind, key: effectKey, amount: effectValue }
        : { kind: effectKind, key: effectKey, value: effectValue }],
    };
    const response = await fetch("/api/npc-behaviors", {
      method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(rule),
    });
    const body = await response.json();
    if (!response.ok) {
      alert(body.error ?? "Could not save this NPC rule.");
      return;
    }
    (document.getElementById("npc-rule-id") as HTMLInputElement).value = "";
    (document.getElementById("npc-rule-action") as HTMLInputElement).value = "";
    (document.getElementById("npc-rule-condition-value") as HTMLInputElement).value = "";
    (document.getElementById("npc-rule-effect-value") as HTMLInputElement).value = "";
    await this.loadNpcBehaviors();
  }

  private async loadLocations(): Promise<void> {
    try {
      const res = await fetch("/api/locations");
      const list = await res.json();
      this.cachedLocations = list;
      const container = document.getElementById("locations-editor-list")!;
      container.innerHTML = "";

      const charLocSelect = document.getElementById("new-char-location") as HTMLSelectElement;
      const genLocSelect = document.getElementById("gen-location") as HTMLSelectElement;
      const connectionsSelect = document.getElementById("new-loc-connections") as HTMLSelectElement;

      if (charLocSelect) charLocSelect.innerHTML = "";
      if (genLocSelect) genLocSelect.innerHTML = "";
      if (connectionsSelect) connectionsSelect.replaceChildren();

      for (const loc of list) {
        const card = document.createElement("div");
        card.className = "codex-card";
        card.id = `codex-card-loc-${loc.id}`;
        card.innerHTML = `
          <div class="codex-card-header">
            <span class="codex-card-title">${loc.name}</span>
            <button class="btn-hud-action" style="padding: 2px 7px; color: var(--accent-rose);" data-del-loc="${loc.id}">✕</button>
          </div>
          <div class="codex-card-desc" style="font-size: 12px; color: var(--text-muted); margin: 6px 0;">${loc.description}</div>
          <div style="font-size: 11px; color: var(--text-dim); border-top: 1px solid var(--border-subtle); padding-top: 6px;">
            Node ID: <code>${loc.id}</code>
          </div>
        `;

        card.querySelector("button")?.addEventListener("click", async (e) => {
          const id = (e.target as HTMLElement).getAttribute("data-del-loc");
          if (id) {
            await fetch(`/api/locations/${id}`, { method: "DELETE" });
            await this.loadLocations();
            this.renderCodexMasterList();
          }
        });

        container.appendChild(card);

        if (charLocSelect) {
          const opt = document.createElement("option");
          opt.value = loc.id;
          opt.innerText = loc.name;
          charLocSelect.appendChild(opt);
        }
        if (genLocSelect) {
          const opt = document.createElement("option");
          opt.value = loc.id;
          opt.innerText = loc.name;
          genLocSelect.appendChild(opt);
        }
        if (connectionsSelect) connectionsSelect.add(new Option(loc.name, loc.id));
      }
      this.populatePlaytestStartLocations(list);
    } catch (e) {
      console.error("Failed to load locations:", e);
    }
  }

  private async onAddLocation(): Promise<void> {
    const idInput = document.getElementById("new-loc-id") as HTMLInputElement;
    const nameInput = document.getElementById("new-loc-name") as HTMLInputElement;
    const descInput = document.getElementById("new-loc-desc") as HTMLInputElement;
    const connectionsSelect = document.getElementById("new-loc-connections") as HTMLSelectElement;

    const id = idInput.value.trim();
    const name = nameInput.value.trim();
    const desc = descInput.value.trim();
    const connectedZones = Array.from(connectionsSelect?.selectedOptions ?? []).map((option) => option.value);

    if (!id || !name) {
      alert("Please provide Location ID and Name.");
      return;
    }

    const response = await fetch("/api/locations", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        id,
        name,
        description: desc || "",
        connected_zones: connectedZones,
      })
    });
    if (!response.ok) {
      alert("Could not save this location.");
      return;
    }
    for (const connectedId of connectedZones) {
      const existing = this.cachedLocations.find((location) => location.id === connectedId);
      if (!existing) continue;
      const reciprocal = new Set(existing.connected_zones ?? []);
      reciprocal.add(id);
      await fetch("/api/locations", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ ...existing, connected_zones: [...reciprocal] }),
      });
    }

    idInput.value = "";
    nameInput.value = "";
    descInput.value = "";

    await this.loadLocations();
    this.renderCodexMasterList();
  }

  // =========================================================================
  // WORKSHOP TRI-PANE ACTIONS
  // =========================================================================

  private async loadStoryletDeck(): Promise<void> {
    try {
      const res = await fetch("/api/storylets");
      const list = await res.json();
      this.cachedStorylets = list;
      const container = document.getElementById("storylet-deck-catalog")!;
      container.innerHTML = "";

      const countBadge = document.getElementById("deck-count-badge");
      if (countBadge) countBadge.innerText = `${list.length} Storylets`;

      for (const st of list) {
        const card = document.createElement("div");
        card.className = "candidate-item-card";
        card.style.cursor = "pointer";
        card.innerHTML = `
          <div class="candidate-title-row">
            <strong style="color: var(--accent-cyan); font-size: 13.5px;">${st.title}</strong>
            <button class="btn-hud-action" style="padding: 2px 7px; color: var(--accent-rose);" data-id="${st.id}">✕</button>
          </div>
          <div class="candidate-metrics-row">
            <span>Location: <strong>${st.location ?? "Any"}</strong></span>
            <span>Cluster: <strong>${st.cluster_type}</strong></span>
            <span>Weight: <strong>${st.urgency_weight}</strong></span>
            <span class="storylet-status ${st.approved_revision === st.revision ? "approved" : "draft"}">${st.approved_revision === st.revision ? "Approved" : "Draft"} · v${st.revision ?? 1}</span>
          </div>
        `;

        // Click card to inspect in manuscript preview
        card.addEventListener("click", (e) => {
          if ((e.target as HTMLElement).tagName === "BUTTON") return;
          this.lastGenerationAudit = null;
          this.previewStoryletInManuscript(st);
        });

        card.querySelector("button")?.addEventListener("click", async (e) => {
          const btn = e.target as HTMLElement;
          const id = btn.getAttribute("data-id");
          if (id) {
            await fetch(`/api/storylets/${id}`, { method: "DELETE" });
            this.loadStoryletDeck();
          }
        });

        container.appendChild(card);
      }
    } catch (e) {
      console.error("Failed to load deck:", e);
    }
  }

  private previewStoryletInManuscript(st: any): void {
    this.lastGeneratedStorylet = st;
    const titleEl = document.getElementById("gen-res-title");
    const openingEl = document.getElementById("gen-res-opening");
    const choicesBox = document.getElementById("gen-res-choices");
    const auditEl = document.getElementById("gen-res-audit");

    if (titleEl) titleEl.textContent = st.title;
    if (openingEl) {
      openingEl.textContent = st.content?.sceneOpening ?? st.content?.text_template ?? "";
      openingEl.classList.remove("manuscript-empty-state");
    }
    const auditBanner = document.getElementById("gen-audit-banner");
    const auditSummary = document.getElementById("gen-audit-summary");
    const auditDetails = document.getElementById("gen-audit-details") as HTMLDetailsElement | null;
    const auditFindings = document.getElementById("gen-audit-findings");
    if (auditEl && auditSummary && auditBanner && auditDetails && auditFindings) {
      const audit = this.lastGenerationAudit;
      const findings = audit?.violations ?? [];
      const counts = summarizeUnslopViolations(findings);
      auditEl.className = `audit-badge ${!audit ? "audit-pending" : audit.passed ? "audit-passed" : "audit-findings"}`;
      auditEl.textContent = !audit ? "DESIGNER REVIEW REQUIRED"
        : audit.passed ? "STYLE SCAN: NO FLAGS" : `STYLE SCAN: ${findings.length} FLAGS`;
      auditBanner.dataset.auditState = !audit ? "pending" : audit.passed ? "passed" : "findings";
      auditSummary.textContent = !audit
        ? "Automated style scan has not run. Review the scene and choices manually."
        : audit.passed
          ? "No listed phrase or pattern was flagged. Designer review is still required."
          : `${findings.length} style finding${findings.length === 1 ? "" : "s"} to review. The source text was left unchanged.`;
      const countLabel = (id: string, label: string, value: number | null) => {
        const element = document.getElementById(id);
        if (element) element.textContent = `${label}: ${value ?? "—"}`;
      };
      countLabel("gen-audit-negative-parallelism", "Negative parallelism", audit ? counts.negativeParallelism : null);
      countLabel("gen-audit-countdowns", "Countdowns", audit ? counts.countdowns : null);
      countLabel("gen-audit-questions", "Self-answered questions", audit ? counts.selfAnsweredQuestions : null);
      countLabel("gen-audit-buzzwords", "Banned words", audit ? counts.buzzwords : null);
      auditFindings.replaceChildren();
      for (const finding of findings) {
        const item = document.createElement("li");
        item.textContent = finding;
        auditFindings.appendChild(item);
      }
      auditDetails.classList.toggle("hidden", !audit || findings.length === 0);
    }
    const approveButton = document.getElementById("btn-approve-storylet") as HTMLButtonElement | null;
    if (approveButton) approveButton.disabled = !st.revision || st.approved_revision === st.revision;

    if (choicesBox) {
      choicesBox.replaceChildren();
      const choices = st.choices ?? [];
      if (!choices.length) {
        const empty = document.createElement("p");
        empty.className = "manuscript-empty-state";
        empty.textContent = "This draft has no choices yet.";
        choicesBox.appendChild(empty);
      }
      for (const c of choices) {
        const card = document.createElement("div");
        card.className = "choice-preview-card";
        const verb = document.createElement("input");
        verb.className = "form-input storylet-choice-verb";
        verb.value = c.systemVerb ?? "ACT";
        verb.setAttribute("aria-label", "Choice action type");
        const label = document.createElement("input");
        label.className = "form-input storylet-choice-label";
        label.value = c.label ?? "";
        label.setAttribute("aria-label", "Choice text");
        const effectsLabel = document.createElement("label");
        effectsLabel.className = "playtest-help";
        effectsLabel.textContent = "Outcome effects (JSON)";
        const effects = document.createElement("textarea");
        effects.className = "form-input storylet-choice-effects";
        effects.value = JSON.stringify(c.effects ?? {}, null, 2);
        effects.rows = 3;
        effects.setAttribute("aria-label", "Choice outcome effects in JSON");
        card.append(verb, label, effectsLabel, effects);
        choicesBox.appendChild(card);
      }
    }
    this.savedManuscriptSnapshot = JSON.stringify({
      title: st.title.trim(),
      opening: (st.content?.sceneOpening ?? st.content?.text_template ?? "").trim(),
      choices: (st.choices ?? []).map((choice: any) => ({
        verb: choice.systemVerb ?? "ACT",
        label: choice.label ?? "",
        effects: JSON.stringify(choice.effects ?? {}, null, 2),
      })),
    });
    document.querySelectorAll("#gen-res-title, #gen-res-opening, #gen-res-choices input, #gen-res-choices textarea")
      .forEach((element) => element.addEventListener("input", () => this.updateApprovalButton()));
  }

  private currentManuscriptSnapshot(): string {
    const choices = [...document.querySelectorAll("#gen-res-choices .choice-preview-card") as NodeListOf<HTMLElement>]
      .map((card) => ({
        verb: card.querySelector<HTMLInputElement>(".storylet-choice-verb")?.value.trim() ?? "",
        label: card.querySelector<HTMLInputElement>(".storylet-choice-label")?.value.trim() ?? "",
        effects: card.querySelector<HTMLTextAreaElement>(".storylet-choice-effects")?.value ?? "{}",
      }));
    return JSON.stringify({
      title: document.getElementById("gen-res-title")?.textContent?.trim() ?? "",
      opening: document.getElementById("gen-res-opening")?.textContent?.trim() ?? "",
      choices,
    });
  }

  private updateApprovalButton(): void {
    const button = document.getElementById("btn-approve-storylet") as HTMLButtonElement | null;
    if (button && this.lastGeneratedStorylet) {
      button.disabled = !this.lastGeneratedStorylet.revision
        || this.lastGeneratedStorylet.approved_revision === this.lastGeneratedStorylet.revision
        || this.currentManuscriptSnapshot() !== this.savedManuscriptSnapshot;
    }
  }

  private async onGenerateStorylet(): Promise<void> {
    const btn = document.getElementById("btn-run-strata-gen") as HTMLButtonElement;
    const location = (document.getElementById("gen-location") as HTMLSelectElement).value;
    const clusterType = (document.getElementById("gen-cluster") as HTMLSelectElement).value;
    const rolesStr = (document.getElementById("gen-roles") as HTMLInputElement).value;
    const precondStr = (document.getElementById("gen-preconditions") as HTMLInputElement).value;
    const outcomesStr = (document.getElementById("gen-outcomes") as HTMLInputElement).value;
    const seed = (document.getElementById("gen-seed") as HTMLInputElement).value;

    let preconditions: unknown[];
    let outcomes: Record<string, unknown>;
    try {
      preconditions = JSON.parse(precondStr);
      outcomes = JSON.parse(outcomesStr);
      if (!Array.isArray(preconditions) || outcomes === null || Array.isArray(outcomes) || typeof outcomes !== "object") throw new Error("Expected a JSON array for triggers and an object for outcomes.");
    } catch (err) {
      alert(err instanceof Error ? err.message : "Trigger and outcome fields contain invalid JSON.");
      btn.disabled = false;
      btn.innerHTML = "<span>⚡</span><span>Create staged generation job</span>";
      return;
    }

    try {
      btn.disabled = true;
      btn.innerHTML = "<span>⏳</span><span>Creating staged job…</span>";
      const res = await fetch("/api/v1/generation-jobs", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          location,
          clusterType,
          characterRoles: rolesStr.split(",").map((s) => s.trim()).filter(Boolean),
          triggerPreconditions: preconditions,
          desiredOutcomes: outcomes,
          seedDetail: seed || undefined,
        })
      });

      const data = await res.json();
      if (!res.ok) throw new Error(data.error ?? "Could not create a generation job.");
      this.generationJob = data.job;
      this.selectedGenerationStage = null;
      this.renderGenerationJob(data.job);
      this.attachGenerationEvents(data.job.id);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      alert(`Could not start generation: ${msg}`);
    } finally {
      btn.disabled = false;
      btn.innerHTML = "<span>⚡</span><span>Create staged generation job</span>";
    }
  }

  private async loadLatestGenerationJob(): Promise<void> {
    try {
      const response = await fetch("/api/v1/generation-jobs");
      if (!response.ok) return;
      const jobs = await response.json() as GenerationJob[];
      if (!jobs.length) return;
      this.generationJob = jobs[0];
      this.selectedGenerationStage = this.generationJob.stages.at(-1)?.stage ?? null;
      this.renderGenerationJob(this.generationJob);
      this.attachGenerationEvents(this.generationJob.id);
    } catch (error) {
      console.error("Could not restore the latest generation job:", error);
    }
  }

  private attachGenerationEvents(jobId: string): void {
    this.generationEvents?.close();
    this.generationEvents = new EventSource(`/api/v1/generation-jobs/${encodeURIComponent(jobId)}/events`);
    this.generationEvents.addEventListener("generation", (event) => {
      try {
        const job = JSON.parse((event as MessageEvent).data) as GenerationJob;
        if (this.generationJob?.id !== job.id) return;
        this.generationJob = job;
        this.renderGenerationJob(job);
      } catch (error) {
        console.error("Invalid generation progress event:", error);
      }
    });
  }

  private renderGenerationJob(job: GenerationJob): void {
    const panel = document.getElementById("generation-job-panel");
    const list = document.getElementById("generation-stage-list");
    const output = document.getElementById("generation-stage-output") as HTMLTextAreaElement | null;
    if (!panel || !list || !output) return;
    panel.classList.remove("hidden");
    const progress = document.getElementById("generation-job-progress");
    if (progress) progress.textContent = `${job.stages.length} / ${GENERATION_STAGES.length}`;
    const status = document.getElementById("generation-job-status");
    if (status) status.textContent = job.failure
      ? `${job.status} at ${job.failure.stage}: ${job.failure.message}`
      : job.status === "running" ? `Generating ${GENERATION_STAGES[job.nextStageIndex]?.title ?? "stage"}…`
        : job.status === "complete" ? "Review all nine stage outputs, then save the final package as a draft."
          : `Job ${job.id.slice(0, 8)} · ${job.status} · revision ${job.revision}`;
    list.replaceChildren();
    for (let index = 0; index < GENERATION_STAGES.length; index++) {
      const stage = GENERATION_STAGES[index];
      const completed = job.stages.some((record) => record.stage === stage.id);
      const isCurrent = index === job.nextStageIndex && job.status !== "complete";
      const item = document.createElement("li");
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.stage = stage.id;
      button.className = `${completed ? "complete " : ""}${this.selectedGenerationStage === stage.id ? "active" : ""}`;
      button.disabled = !completed;
      const label = document.createElement("span");
      label.textContent = `${index + 1}. ${stage.title}`;
      const state = document.createElement("span");
      state.textContent = completed ? "Complete" : isCurrent ? job.status === "failed" ? "Failed" : "Next" : "Pending";
      button.append(label, state);
      item.appendChild(button);
      list.appendChild(item);
    }
    const selected = job.stages.find((record) => record.stage === this.selectedGenerationStage);
    output.value = selected ? JSON.stringify(selected.artifact, null, 2) : "Select a completed stage to inspect its output.";
    output.disabled = !selected || job.status === "running";
    const provenance = document.getElementById("generation-stage-provenance");
    const provenanceDetails = document.getElementById("generation-stage-provenance-details");
    if (provenance && provenanceDetails) {
      provenance.classList.toggle("hidden", !selected);
      provenanceDetails.replaceChildren();
      if (selected) {
        const provider = document.createElement("p");
        provider.textContent = `Provider: ${selected.source} · ${selected.model} · ${selected.promptVersion}`;
        provenanceDetails.appendChild(provider);
        for (const resource of selected.craftResources ?? []) {
          const row = document.createElement("p");
          row.textContent = `${resource.id}/${resource.version} · ${resource.source} · source SHA-256 ${resource.sourceHash} · prompt SHA-256 ${resource.promptHash}`;
          provenanceDetails.appendChild(row);
        }
      }
    }
    const saveStage = document.getElementById("btn-save-generation-stage") as HTMLButtonElement | null;
    if (saveStage) saveStage.disabled = !selected || job.status === "running";
    const nextStage = document.getElementById("btn-next-generation-stage") as HTMLButtonElement | null;
    if (nextStage) {
      nextStage.disabled = job.status === "running" || job.status === "failed" || job.status === "complete";
      nextStage.textContent = job.status === "running" ? "Generating stage…"
        : `Generate stage ${Math.min(job.nextStageIndex + 1, GENERATION_STAGES.length)} of ${GENERATION_STAGES.length}`;
    }
    const saveDraft = document.getElementById("btn-save-generation-draft") as HTMLButtonElement | null;
    if (saveDraft) {
      saveDraft.disabled = job.status !== "complete" || Boolean(job.draftStoryletId);
      if (job.draftStoryletId) saveDraft.textContent = "Draft saved to Codex";
    }
  }

  private async onAdvanceGenerationStage(): Promise<void> {
    if (!this.generationJob) return;
    const button = document.getElementById("btn-next-generation-stage") as HTMLButtonElement;
    button.disabled = true;
    try {
      const response = await fetch(`/api/v1/generation-jobs/${encodeURIComponent(this.generationJob.id)}/advance`, {
        method: "POST", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ expectedRevision: this.generationJob.revision }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Stage generation failed.");
      this.generationJob = body.job;
      this.selectedGenerationStage = body.job.stages.at(-1)?.stage ?? null;
      this.renderGenerationJob(body.job);
    } catch (error) {
      await this.refreshGenerationJob();
      alert(error instanceof Error ? error.message : String(error));
    } finally {
      if (this.generationJob) this.renderGenerationJob(this.generationJob);
    }
  }

  private async refreshGenerationJob(): Promise<void> {
    if (!this.generationJob) return;
    const response = await fetch(`/api/v1/generation-jobs/${encodeURIComponent(this.generationJob.id)}`);
    const body = await response.json();
    if (response.ok) {
      this.generationJob = body.job;
      this.renderGenerationJob(body.job);
    }
  }

  private async onSaveGenerationStage(): Promise<void> {
    if (!this.generationJob || !this.selectedGenerationStage) return;
    const output = document.getElementById("generation-stage-output") as HTMLTextAreaElement;
    let artifact: unknown;
    try { artifact = JSON.parse(output.value); }
    catch { alert("Stage output must remain valid JSON."); return; }
    try {
      const response = await fetch(`/api/v1/generation-jobs/${encodeURIComponent(this.generationJob.id)}/stages/${encodeURIComponent(this.selectedGenerationStage)}`, {
        method: "PUT", headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ artifact, expectedRevision: this.generationJob.revision }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not save this stage edit.");
      this.generationJob = body.job;
      this.selectedGenerationStage = body.job.stages.at(-1)?.stage ?? null;
      this.renderGenerationJob(body.job);
    } catch (error) {
      alert(error instanceof Error ? error.message : String(error));
    }
  }

  private async onSaveGenerationDraft(): Promise<void> {
    if (!this.generationJob) return;
    const response = await fetch(`/api/v1/generation-jobs/${encodeURIComponent(this.generationJob.id)}/draft`, { method: "POST" });
    const body = await response.json();
    if (!response.ok) {
      alert(body.error ?? "Could not build the draft from this review package.");
      return;
    }
    this.lastGenerationAudit = body.audit;
    this.generationJob = body.job ?? this.generationJob;
    this.previewStoryletInManuscript(body.storylet);
    if (this.generationJob) this.renderGenerationJob(this.generationJob);
    await this.loadStoryletDeck();
  }

  private async onSaveGeneratedStorylet(): Promise<void> {
    if (!this.lastGeneratedStorylet) return;
    if (this.lastGeneratedStorylet.revision && this.currentManuscriptSnapshot() === this.savedManuscriptSnapshot) {
      alert("No unsaved edits to save.");
      return;
    }
    if (!this.readStoryletManuscriptEdits()) return;
    const response = await fetch("/api/storylets", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(this.lastGeneratedStorylet)
    });
    const body = await response.json();
    if (!response.ok) {
      alert(body.error ?? "Could not save this storylet draft.");
      return;
    }
    this.lastGeneratedStorylet = body.storylet;
    this.previewStoryletInManuscript(body.storylet);
    alert(`Saved '${body.storylet.title}' as draft revision ${body.storylet.revision}.`);
    await this.loadStoryletDeck();
  }

  private readStoryletManuscriptEdits(): boolean {
    if (!this.lastGeneratedStorylet) return false;
    const title = document.getElementById("gen-res-title")?.textContent?.trim();
    const opening = document.getElementById("gen-res-opening")?.textContent?.trim();
    if (!title || !opening) {
      alert("A storylet needs a title and scene opening before it can be saved.");
      return false;
    }
    this.lastGeneratedStorylet.title = title;
    this.lastGeneratedStorylet.content = { ...this.lastGeneratedStorylet.content, text_template: opening, sceneOpening: opening };
    const cards = [...(document.querySelectorAll("#gen-res-choices .choice-preview-card") as NodeListOf<HTMLElement>)];
    try {
      this.lastGeneratedStorylet.choices = cards.map((card, index) => {
        const verb = card.querySelector<HTMLInputElement>(".storylet-choice-verb")?.value.trim();
        const label = card.querySelector<HTMLInputElement>(".storylet-choice-label")?.value.trim();
        const rawEffects = card.querySelector<HTMLTextAreaElement>(".storylet-choice-effects")?.value ?? "{}";
        const effects = JSON.parse(rawEffects);
        if (!verb || !label || effects === null || Array.isArray(effects) || typeof effects !== "object") {
          throw new Error(`Choice ${index + 1} needs an action type, label, and JSON object of effects.`);
        }
        return { ...this.lastGeneratedStorylet.choices[index], systemVerb: verb, label, effects };
      });
    } catch (error) {
      alert(error instanceof Error ? error.message : "Choice effects must be valid JSON objects.");
      return false;
    }
    return true;
  }

  private async onApproveGeneratedStorylet(): Promise<void> {
    if (!this.lastGeneratedStorylet?.revision) {
      alert("Save the draft first. Approval applies to a saved revision.");
      return;
    }
    if (this.currentManuscriptSnapshot() !== this.savedManuscriptSnapshot) {
      alert("Save your edits as a new draft revision before approving.");
      return;
    }
    const response = await fetch(`/api/storylets/${encodeURIComponent(this.lastGeneratedStorylet.id)}/approve`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ expectedRevision: this.lastGeneratedStorylet.revision }),
    });
    const body = await response.json();
    if (!response.ok) {
      alert(body.error ?? "Could not approve this revision.");
      return;
    }
    this.lastGeneratedStorylet = body.storylet;
    this.previewStoryletInManuscript(body.storylet);
    await this.loadStoryletDeck();
    alert(`Approved '${body.storylet.title}' revision ${body.storylet.revision}.`);
  }

  // =========================================================================
  // DIRECTOR ACTIONS
  // =========================================================================

  private async refreshDirectorEvaluation(): Promise<void> {
    const candidatesList = document.getElementById("director-candidates-list");
    const jevBox = document.getElementById("director-jev-box");
    const status = document.getElementById("director-session-status");
    if (!candidatesList || !jevBox) return;
    candidatesList.replaceChildren();
    if (!this.playtestSessionId) {
      candidatesList.textContent = "Start a clean playtest to evaluate this project's storylets.";
      jevBox.textContent = "No active session. The director does not infer state from the authoring database.";
      if (status) status.textContent = "No active playtest";
      return;
    }
    try {
      const res = await fetch(`/api/v1/playtests/${encodeURIComponent(this.playtestSessionId)}/director`);
      const data = await res.json();
      if (!res.ok) throw new Error(data.error ?? "Director evaluation failed.");
      if (status && this.playtestSnapshot) {
        status.textContent = `Turn ${this.playtestSnapshot.world.turn} · Revision ${this.playtestSnapshot.world.revision}`;
      }
      for (const candidate of data.candidates ?? []) {
        const storylet = this.cachedStorylets.find((item) => item.id === candidate.storylet_id);
        const card = document.createElement("div");
        card.className = `candidate-item-card ${candidate.storylet_id === data.selected ? "top-pick" : ""}`;
        const title = document.createElement("strong");
        title.className = "candidate-title";
        title.textContent = `${candidate.storylet_id === data.selected ? "★ " : ""}${storylet?.title ?? candidate.storylet_id}`;
        const score = document.createElement("span");
        score.className = "shift-pill";
        score.textContent = `Weight: ${Number(candidate.final_weight).toFixed(1)}`;
        const reason = document.createElement("div");
        reason.className = "candidate-metrics-row";
        reason.textContent = candidate.eligible
          ? `Eligible · roles: ${Object.entries(candidate.bound_roles ?? {}).map(([role, actor]) => `${role} → ${actor}`).join(", ") || "none required"}`
          : `Rejected · ${candidate.rejection?.reason ?? "ineligible"}`;
        const heading = document.createElement("div");
        heading.className = "candidate-title-row";
        heading.append(title, score);
        card.append(heading, reason);
        candidatesList.appendChild(card);
      }
      if (!candidatesList.childElementCount) {
        candidatesList.textContent = "This scenario contains no approved storylets.";
      }

      jevBox.replaceChildren();
      const selected = document.createElement("div");
      selected.textContent = data.selected ? `Selected: ${data.selected}` : "No action: no eligible storylet fits the current state.";
      const method = document.createElement("p");
      method.className = "playtest-help";
      method.textContent = "Selection uses the deterministic Rust baseline. Scene Preview can compare a local embedding baseline; its probabilities remain uncalibrated until reviewed training and held-out evaluation.";
      jevBox.append(selected, method);
      document.getElementById("backbuffer-count")!.innerText = "—";
      document.getElementById("aftermath-count")!.innerText = "—";
    } catch (e) {
      console.error("Director refresh failed:", e);
    }
  }

  private async onPlayDirectorSelection(): Promise<void> {
    if (!this.playtestSessionId) {
      alert("Start a clean playtest first, then open the selected scene from the Playtest view.");
    }
    document.querySelector<HTMLButtonElement>('.nav-item-btn[data-nav="view-playtest"]')?.click();
    await this.refreshPlaytest();
  }

  private onOpenBranchTools(): void {
    document.querySelector<HTMLButtonElement>('.nav-item-btn[data-nav="view-butterfly"]')?.click();
    document.getElementById("butterfly-canvas")?.scrollIntoView({ behavior: "smooth", block: "center" });
  }

  // =========================================================================
  // BUTTERFLY ACTIONS
  // =========================================================================

  private async refreshButterflyChain(): Promise<void> {
    const canvas = document.getElementById("butterfly-canvas") as HTMLCanvasElement | null;
    const status = document.getElementById("butterfly-graph-status");
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const ratio = window.devicePixelRatio || 1;
    canvas.width = Math.max(1, Math.floor(rect.width * ratio));
    canvas.height = Math.max(1, Math.floor(rect.height * ratio));
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.scale(ratio, ratio);
    const width = rect.width;
    const height = rect.height;
    ctx.clearRect(0, 0, width, height);
    const events = this.playtestSnapshot?.world?.events ?? [];
    if (status) status.textContent = events.length ? `${events.length} events · click a node for details` : "Start a playtest to inspect its event graph.";
    if (!events.length) return;
    const points = this.butterflyPoints(events, width, height);
    const byId = new Map(points.map((point) => [point.id, point]));
    ctx.lineWidth = 1.5;
    for (const event of events) {
      const child = byId.get(event.id);
      if (!child) continue;
      for (const parentId of event.caused_by ?? []) {
        const parent = byId.get(parentId);
        if (!parent) continue;
        ctx.beginPath();
        ctx.strokeStyle = "rgba(99, 179, 237, .55)";
        ctx.moveTo(parent.x, parent.y);
        ctx.bezierCurveTo((parent.x + child.x) / 2, parent.y, (parent.x + child.x) / 2, child.y, child.x, child.y);
        ctx.stroke();
      }
    }
    for (const point of points) {
      ctx.beginPath();
      ctx.fillStyle = point.id === this.selectedButterflyEventId ? "#ffffff" : point.depth ? "#f6bd60" : "#72c7d8";
      ctx.arc(point.x, point.y, point.id === this.selectedButterflyEventId ? 9 : 7, 0, Math.PI * 2);
      ctx.fill();
      ctx.fillStyle = "#d7deea";
      ctx.font = "11px Inter, sans-serif";
      ctx.fillText(`${point.turn} · ${point.actor}`, point.x + 11, point.y + 4);
    }
  }

  private butterflyPoints(events: any[], width: number, height: number): Array<{ id: string; x: number; y: number; turn: number; actor: string; depth: number }> {
    const actors = [...new Set(events.map((event) => event.actor_id))];
    const actorLane = new Map(actors.map((actor, index) => [actor, index]));
    const byId = new Map(events.map((event) => [event.id, event]));
    const depths = new Map<string, number>();
    const depthOf = (event: any, seen = new Set<string>()): number => {
      if (depths.has(event.id)) return depths.get(event.id)!;
      if (seen.has(event.id)) return 0;
      seen.add(event.id);
      const value = Math.min(6, Math.max(0, ...(event.caused_by ?? []).map((id: string) => {
        const parent = byId.get(id);
        return parent ? depthOf(parent, seen) + 1 : 0;
      })));
      depths.set(event.id, value);
      return value;
    };
    const inset = 24;
    const laneHeight = Math.max(1, (height - inset * 2) / Math.max(1, actors.length));
    return events.map((event, index) => ({
      id: event.id,
      x: inset + (events.length < 2 ? 0 : index * (width - inset * 2) / (events.length - 1)),
      y: inset + (actorLane.get(event.actor_id) ?? 0) * laneHeight + laneHeight / 2,
      turn: event.turn,
      actor: event.actor_id,
      depth: depthOf(event),
    }));
  }

  private async onRunButterflyTick(): Promise<void> {
    const btn = document.getElementById("btn-run-butterfly-tick") as HTMLButtonElement;
    if (!this.playtestSessionId || !this.playtestSnapshot) {
      alert("Start a playtest before advancing autonomous world events.");
      return;
    }
    btn.disabled = true;
    btn.innerHTML = "<span>⏱️</span><span>Advancing one world turn…</span>";

    try {
      await this.submitPlaytestAction("wait", []);
    } catch (e) {
      console.error("Butterfly tick failed:", e);
    } finally {
      btn.disabled = false;
      btn.innerHTML = "<span>⏱️</span><span>Advance one world turn</span>";
    }
  }

  // =========================================================================
  // GENERIC RUST-BACKED PLAYTEST
  // =========================================================================

  private async loadPlaytestLocations(): Promise<void> {
    try {
      const [locationsResponse, qualitiesResponse, charactersResponse] = await Promise.all([
        fetch("/api/locations"), fetch("/api/qualities"), fetch("/api/characters"),
      ]);
      if (!locationsResponse.ok || !qualitiesResponse.ok || !charactersResponse.ok) throw new Error("Could not load project records for the scenario editor.");
      this.cachedLocations = await locationsResponse.json();
      this.cachedQualities = await qualitiesResponse.json();
      this.cachedCharacters = await charactersResponse.json();
      this.populatePlaytestStartLocations(this.cachedLocations);
      this.populateAttributeSelectOptions();
      this.updateScenarioDiagnostics();
      this.renderScenarioStateFields();
      await this.loadPlaytestScenarios();
    } catch (error) {
      console.error("Failed to load playtest locations:", error);
    }
  }

  private renderScenarioStateFields(initialQualities: Record<string, unknown> = {}, actorLocations: Record<string, string> = {}): void {
    const qualityFields = document.getElementById("scenario-initial-quality-fields");
    const actorFields = document.getElementById("scenario-actor-location-fields");
    if (!qualityFields || !actorFields) return;
    qualityFields.replaceChildren();
    actorFields.replaceChildren();

    if (!this.cachedQualities.length) {
      qualityFields.append(Object.assign(document.createElement("span"), { className: "playtest-help", textContent: "Add qualities in the Codex to set their starting values." }));
    } else {
      for (const quality of this.cachedQualities) {
        const label = document.createElement("label");
        label.className = "scenario-state-row";
        const name = document.createElement("span");
        name.textContent = quality.name;
        const input = document.createElement("input");
        input.className = "form-input";
        input.type = "number";
        input.step = "any";
        input.dataset.quality = quality.name;
        input.setAttribute("aria-label", `Starting value for ${quality.name}`);
        input.placeholder = `Default ${quality.value}`;
        const value = initialQualities[quality.name];
        input.value = typeof value === "number" ? String(value) : "";
        label.append(name, input);
        qualityFields.append(label);
      }
      const hint = document.createElement("span");
      hint.className = "playtest-help";
      hint.textContent = "Leave a value blank to use the Codex default.";
      qualityFields.append(hint);
    }

    if (!this.cachedCharacters.length) {
      actorFields.append(Object.assign(document.createElement("span"), { className: "playtest-help", textContent: "Add characters in the Codex to place them in this scenario." }));
    } else {
      for (const character of this.cachedCharacters) {
        const label = document.createElement("label");
        label.className = "scenario-state-row";
        const name = document.createElement("span");
        name.textContent = character.name;
        const select = document.createElement("select");
        select.className = "form-select";
        select.dataset.actor = character.id;
        select.setAttribute("aria-label", `Starting location for ${character.name}`);
        select.add(new Option("Use character's Codex location", ""));
        for (const location of this.cachedLocations) select.add(new Option(location.name, location.id));
        select.value = actorLocations[character.id] ?? "";
        label.append(name, select);
        actorFields.append(label);
      }
    }
  }

  private async loadPlaytestScenarios(): Promise<void> {
    const scenarioSelect = document.getElementById("playtest-scenario-select") as HTMLSelectElement | null;
    const storyletSelect = document.getElementById("scenario-storylet-ids") as HTMLSelectElement | null;
    if (!scenarioSelect || !storyletSelect) return;
    try {
      const [scenarioResponse, storyletResponse] = await Promise.all([fetch("/api/v1/scenarios"), fetch("/api/storylets")]);
      const [scenarios, storylets] = await Promise.all([scenarioResponse.json(), storyletResponse.json()]);
      if (!scenarioResponse.ok || !storyletResponse.ok) throw new Error("Could not load scenarios or approved storylets.");
      this.cachedScenarios = scenarios;
      this.cachedStorylets = storylets;
      this.populateAttributeSelectOptions();
      const approved = storylets.filter((storylet: any) => storylet.revision != null && storylet.approved_revision === storylet.revision);
      storyletSelect.replaceChildren();
      if (!approved.length) {
        storyletSelect.add(new Option("No approved storylets yet", ""));
      } else {
        for (const storylet of approved) storyletSelect.add(new Option(storylet.title, storylet.id, false, true));
      }
      const previous = scenarioSelect.value;
      scenarioSelect.replaceChildren(new Option("New scenario / quick start", ""));
      for (const scenario of scenarios) scenarioSelect.add(new Option(`${scenario.name} · ${scenario.id}`, scenario.id));
      if (scenarios.some((scenario: any) => scenario.id === previous)) {
        scenarioSelect.value = previous;
        const selected = scenarios.find((scenario: any) => scenario.id === previous);
        if (selected) this.populateScenarioEditor(selected);
      } else if (!previous) {
        this.newPlaytestScenario();
      }
    } catch (error) {
      console.error("Could not load authored scenarios:", error);
    }
  }

  private populateScenarioEditor(scenario: any): void {
    (document.getElementById("scenario-id") as HTMLInputElement).value = scenario.id;
    (document.getElementById("scenario-name") as HTMLInputElement).value = scenario.name;
    (document.getElementById("playtest-start-location") as HTMLSelectElement).value = scenario.player_start_location;
    (document.getElementById("scenario-seed") as HTMLInputElement).value = String(scenario.seed);

    // Populate start_location_policy
    const startPolicy = scenario.start_location_policy;
    const startLocMode = document.getElementById("scenario-start-location-mode") as HTMLSelectElement | null;
    if (startLocMode) startLocMode.value = startPolicy?.mode || "fixed";
    const startLocKey = document.getElementById("scenario-start-location-attr-key") as HTMLSelectElement | null;
    if (startLocKey) startLocKey.value = startPolicy?.attribute_key || "";
    const startLocVal = document.getElementById("scenario-start-location-attr-val") as HTMLInputElement | null;
    if (startLocVal) startLocVal.value = startPolicy?.attribute_value !== undefined ? String(startPolicy.attribute_value) : "";
    this.updateScenarioStartLocationVisibility();

    // Populate actor_population_policy
    const popPolicy = scenario.actor_population_policy;
    const popMode = document.getElementById("scenario-actor-pop-mode") as HTMLSelectElement | null;
    if (popMode) popMode.value = popPolicy?.mode || "all";
    const popCount = document.getElementById("scenario-actor-pop-count") as HTMLInputElement | null;
    if (popCount) popCount.value = popPolicy?.count !== undefined ? String(popPolicy.count) : "";
    const popKey = document.getElementById("scenario-actor-pop-attr-key") as HTMLSelectElement | null;
    if (popKey) popKey.value = popPolicy?.attribute_key || "";
    const popVal = document.getElementById("scenario-actor-pop-attr-val") as HTMLInputElement | null;
    if (popVal) popVal.value = popPolicy?.attribute_value !== undefined ? String(popPolicy.attribute_value) : "";
    this.updateScenarioActorPopulationVisibility();

    // Populate actor location_policy
    const locPolicy = popPolicy?.location_policy;
    const locMode = document.getElementById("scenario-actor-loc-mode") as HTMLSelectElement | null;
    if (locMode) locMode.value = locPolicy?.mode || "fixed";
    const locFixed = document.getElementById("scenario-actor-loc-fixed-select") as HTMLSelectElement | null;
    if (locFixed) locFixed.value = locPolicy?.location_id || "";
    const locKey = document.getElementById("scenario-actor-loc-attr-key") as HTMLSelectElement | null;
    if (locKey) locKey.value = locPolicy?.attribute_key || "";
    const locVal = document.getElementById("scenario-actor-loc-attr-val") as HTMLInputElement | null;
    if (locVal) locVal.value = locPolicy?.attribute_value !== undefined ? String(locPolicy.attribute_value) : "";
    this.updateScenarioActorLocationVisibility();

    this.renderScenarioStateFields(scenario.initial_qualities ?? {}, scenario.actor_locations ?? {});
    const storyletSelect = document.getElementById("scenario-storylet-ids") as HTMLSelectElement;
    for (const option of [...storyletSelect.options]) option.selected = scenario.storylet_ids.includes(option.value);
    const scenarioSelect = document.getElementById("playtest-scenario-select") as HTMLSelectElement;
    scenarioSelect.value = scenario.id;
    (document.getElementById("btn-delete-playtest-scenario") as HTMLButtonElement).disabled = false;
    (document.getElementById("btn-save-playtest-scenario") as HTMLButtonElement).textContent = "Update scenario";
    (document.getElementById("scenario-validation-message") as HTMLElement).textContent = "Playtests use the last saved scenario revision.";
    this.updatePlaytestStartButton();
    this.updateScenarioDiagnostics();
  }

  private newPlaytestScenario(): void {
    (document.getElementById("playtest-scenario-select") as HTMLSelectElement).value = "";
    (document.getElementById("scenario-id") as HTMLInputElement).value = "";
    (document.getElementById("scenario-name") as HTMLInputElement).value = "";
    (document.getElementById("scenario-seed") as HTMLInputElement).value = "1";

    const startLocMode = document.getElementById("scenario-start-location-mode") as HTMLSelectElement | null;
    if (startLocMode) startLocMode.value = "fixed";
    const startLocKey = document.getElementById("scenario-start-location-attr-key") as HTMLSelectElement | null;
    if (startLocKey) startLocKey.value = "";
    const startLocVal = document.getElementById("scenario-start-location-attr-val") as HTMLInputElement | null;
    if (startLocVal) startLocVal.value = "";
    this.updateScenarioStartLocationVisibility();

    const popMode = document.getElementById("scenario-actor-pop-mode") as HTMLSelectElement | null;
    if (popMode) popMode.value = "all";
    const popCount = document.getElementById("scenario-actor-pop-count") as HTMLInputElement | null;
    if (popCount) popCount.value = "";
    const popKey = document.getElementById("scenario-actor-pop-attr-key") as HTMLSelectElement | null;
    if (popKey) popKey.value = "";
    const popVal = document.getElementById("scenario-actor-pop-attr-val") as HTMLInputElement | null;
    if (popVal) popVal.value = "";
    this.updateScenarioActorPopulationVisibility();

    const locMode = document.getElementById("scenario-actor-loc-mode") as HTMLSelectElement | null;
    if (locMode) locMode.value = "fixed";
    const locFixed = document.getElementById("scenario-actor-loc-fixed-select") as HTMLSelectElement | null;
    if (locFixed) locFixed.value = "";
    const locKey = document.getElementById("scenario-actor-loc-attr-key") as HTMLSelectElement | null;
    if (locKey) locKey.value = "";
    const locVal = document.getElementById("scenario-actor-loc-attr-val") as HTMLInputElement | null;
    if (locVal) locVal.value = "";
    this.updateScenarioActorLocationVisibility();

    this.renderScenarioStateFields();
    for (const option of [...(document.getElementById("scenario-storylet-ids") as HTMLSelectElement).options]) option.selected = Boolean(option.value);
    (document.getElementById("btn-delete-playtest-scenario") as HTMLButtonElement).disabled = true;
    (document.getElementById("btn-save-playtest-scenario") as HTMLButtonElement).textContent = "Save scenario";
    (document.getElementById("scenario-validation-message") as HTMLElement).textContent = "Enter an ID and name, then save this scenario to reuse its starting state.";
    this.updatePlaytestStartButton();
    this.updateScenarioDiagnostics();
  }


  private bindScenarioPolicyControls(): void {
    const startLocMode = document.getElementById("scenario-start-location-mode") as HTMLSelectElement | null;
    startLocMode?.addEventListener("change", () => {
      this.updateScenarioStartLocationVisibility();
      this.updateScenarioDiagnostics();
    });

    const startLocKey = document.getElementById("scenario-start-location-attr-key") as HTMLSelectElement | null;
    startLocKey?.addEventListener("change", () => this.updateScenarioDiagnostics());
    const startLocVal = document.getElementById("scenario-start-location-attr-val") as HTMLInputElement | null;
    startLocVal?.addEventListener("input", () => this.updateScenarioDiagnostics());
    const startLocFixed = document.getElementById("playtest-start-location") as HTMLSelectElement | null;
    startLocFixed?.addEventListener("change", () => this.updateScenarioDiagnostics());

    const actorPopMode = document.getElementById("scenario-actor-pop-mode") as HTMLSelectElement | null;
    actorPopMode?.addEventListener("change", () => {
      this.updateScenarioActorPopulationVisibility();
      this.updateScenarioDiagnostics();
    });

    const actorPopCount = document.getElementById("scenario-actor-pop-count") as HTMLInputElement | null;
    actorPopCount?.addEventListener("input", () => this.updateScenarioDiagnostics());
    const actorPopKey = document.getElementById("scenario-actor-pop-attr-key") as HTMLSelectElement | null;
    actorPopKey?.addEventListener("change", () => this.updateScenarioDiagnostics());
    const actorPopVal = document.getElementById("scenario-actor-pop-attr-val") as HTMLInputElement | null;
    actorPopVal?.addEventListener("input", () => this.updateScenarioDiagnostics());

    const actorLocMode = document.getElementById("scenario-actor-loc-mode") as HTMLSelectElement | null;
    actorLocMode?.addEventListener("change", () => {
      this.updateScenarioActorLocationVisibility();
      this.updateScenarioDiagnostics();
    });

    const actorLocFixed = document.getElementById("scenario-actor-loc-fixed-select") as HTMLSelectElement | null;
    actorLocFixed?.addEventListener("change", () => this.updateScenarioDiagnostics());
    const actorLocKey = document.getElementById("scenario-actor-loc-attr-key") as HTMLSelectElement | null;
    actorLocKey?.addEventListener("change", () => this.updateScenarioDiagnostics());
    const actorLocVal = document.getElementById("scenario-actor-loc-attr-val") as HTMLInputElement | null;
    actorLocVal?.addEventListener("input", () => this.updateScenarioDiagnostics());

    const seedInput = document.getElementById("scenario-seed") as HTMLInputElement | null;
    seedInput?.addEventListener("input", () => this.updateScenarioDiagnostics());

    const storyletSelect = document.getElementById("scenario-storylet-ids") as HTMLSelectElement | null;
    storyletSelect?.addEventListener("change", () => this.updateScenarioDiagnostics());

    document.getElementById("scenario-actor-location-fields")?.addEventListener("change", () => this.updateScenarioDiagnostics());
  }

  private updateScenarioStartLocationVisibility(): void {
    const mode = (document.getElementById("scenario-start-location-mode") as HTMLSelectElement | null)?.value || "fixed";
    const fixedLabel = document.getElementById("scenario-start-location-fixed-label");
    const attrControls = document.getElementById("scenario-start-location-attr-controls");
    if (fixedLabel) fixedLabel.style.display = mode === "fixed" ? "grid" : "none";
    if (attrControls) attrControls.style.display = mode === "by_attribute" ? "grid" : "none";
  }

  private updateScenarioActorPopulationVisibility(): void {
    const mode = (document.getElementById("scenario-actor-pop-mode") as HTMLSelectElement | null)?.value || "all";
    const countLabel = document.getElementById("scenario-actor-pop-count-label");
    const attrControls = document.getElementById("scenario-actor-pop-attr-controls");
    if (countLabel) countLabel.style.display = mode === "random" ? "grid" : "none";
    if (attrControls) attrControls.style.display = mode === "by_attribute" ? "grid" : "none";
  }

  private updateScenarioActorLocationVisibility(): void {
    const mode = (document.getElementById("scenario-actor-loc-mode") as HTMLSelectElement | null)?.value || "fixed";
    const fixedLabel = document.getElementById("scenario-actor-loc-fixed-label");
    const attrControls = document.getElementById("scenario-actor-loc-attr-controls");
    const indivGroup = document.getElementById("scenario-actor-locations-group");
    if (fixedLabel) fixedLabel.style.display = mode === "fixed" ? "grid" : "none";
    if (attrControls) attrControls.style.display = mode === "by_attribute" ? "grid" : "none";
    if (indivGroup) indivGroup.style.display = mode === "fixed" ? "block" : "none";
  }

  private populateAttributeSelectOptions(): void {
    const locAttrKeys = new Set<string>(["id", "name", "description"]);
    for (const loc of this.cachedLocations) {
      if (loc.environmental_params) {
        for (const k of Object.keys(loc.environmental_params)) locAttrKeys.add(k);
      }
      if (loc.devices) {
        for (const k of Object.keys(loc.devices)) locAttrKeys.add(k);
      }
    }
    const sortedLocKeys = [...locAttrKeys].sort();

    const startLocKey = document.getElementById("scenario-start-location-attr-key") as HTMLSelectElement | null;
    if (startLocKey) {
      const prev = startLocKey.value;
      startLocKey.replaceChildren(new Option("Select location attribute", ""));
      for (const k of sortedLocKeys) startLocKey.add(new Option(k, k));
      if (sortedLocKeys.includes(prev)) startLocKey.value = prev;
    }

    const actorLocKey = document.getElementById("scenario-actor-loc-attr-key") as HTMLSelectElement | null;
    if (actorLocKey) {
      const prev = actorLocKey.value;
      actorLocKey.replaceChildren(new Option("Select location attribute", ""));
      for (const k of sortedLocKeys) actorLocKey.add(new Option(k, k));
      if (sortedLocKeys.includes(prev)) actorLocKey.value = prev;
    }

    const actorFixedLocSelect = document.getElementById("scenario-actor-loc-fixed-select") as HTMLSelectElement | null;
    if (actorFixedLocSelect) {
      const prev = actorFixedLocSelect.value;
      actorFixedLocSelect.replaceChildren(new Option("Use individual or Codex location", ""));
      for (const loc of this.cachedLocations) actorFixedLocSelect.add(new Option(loc.name, loc.id));
      if (this.cachedLocations.some((loc: any) => loc.id === prev)) actorFixedLocSelect.value = prev;
    }

    const charAttrKeys = new Set<string>(["id", "name", "role", "home_location", "current_location", "agenda", "red_line"]);
    for (const char of this.cachedCharacters) {
      if (char.status_axes) {
        for (const k of Object.keys(char.status_axes)) charAttrKeys.add(k);
      }
      if (char.qualities) {
        for (const k of Object.keys(char.qualities)) charAttrKeys.add(k);
      }
      if (char.needs) {
        for (const k of Object.keys(char.needs)) charAttrKeys.add(k);
      }
    }
    const sortedCharKeys = [...charAttrKeys].sort();

    const actorPopKey = document.getElementById("scenario-actor-pop-attr-key") as HTMLSelectElement | null;
    if (actorPopKey) {
      const prev = actorPopKey.value;
      actorPopKey.replaceChildren(new Option("Select character attribute", ""));
      for (const k of sortedCharKeys) actorPopKey.add(new Option(k, k));
      if (sortedCharKeys.includes(prev)) actorPopKey.value = prev;
    }
  }

  private updateScenarioDiagnostics(): void {
    const actorsPerLocContainer = document.getElementById("scenario-actors-per-location");
    const roleAvailContainer = document.getElementById("scenario-storylet-role-availability");
    const summarySpan = document.getElementById("scenario-resolved-summary");
    if (!actorsPerLocContainer || !roleAvailContainer) return;

    const seed = Number((document.getElementById("scenario-seed") as HTMLInputElement | null)?.value || "1");
    const startLocMode = (document.getElementById("scenario-start-location-mode") as HTMLSelectElement | null)?.value || "fixed";
    const startLocFixed = (document.getElementById("playtest-start-location") as HTMLSelectElement | null)?.value || "";
    const startLocKey = (document.getElementById("scenario-start-location-attr-key") as HTMLSelectElement | null)?.value || "";
    const startLocVal = (document.getElementById("scenario-start-location-attr-val") as HTMLInputElement | null)?.value || "";

    const actorPopMode = (document.getElementById("scenario-actor-pop-mode") as HTMLSelectElement | null)?.value || "all";
    const actorPopCountVal = (document.getElementById("scenario-actor-pop-count") as HTMLInputElement | null)?.value;
    const actorPopKey = (document.getElementById("scenario-actor-pop-attr-key") as HTMLSelectElement | null)?.value || "";
    const actorPopVal = (document.getElementById("scenario-actor-pop-attr-val") as HTMLInputElement | null)?.value || "";

    const actorLocMode = (document.getElementById("scenario-actor-loc-mode") as HTMLSelectElement | null)?.value || "fixed";
    const actorLocFixed = (document.getElementById("scenario-actor-loc-fixed-select") as HTMLSelectElement | null)?.value || "";
    const actorLocKey = (document.getElementById("scenario-actor-loc-attr-key") as HTMLSelectElement | null)?.value || "";
    const actorLocVal = (document.getElementById("scenario-actor-loc-attr-val") as HTMLInputElement | null)?.value || "";

    const individualActorLocations: Record<string, string> = {};
    for (const select of [...(document.getElementById("scenario-actor-location-fields")?.querySelectorAll<HTMLSelectElement>("select[data-actor]") ?? [])]) {
      if (select.value) individualActorLocations[select.dataset.actor!] = select.value;
    }

    const storyletSelect = document.getElementById("scenario-storylet-ids") as HTMLSelectElement | null;
    const selectedStoryletIds = storyletSelect ? [...storyletSelect.selectedOptions].map((o) => o.value).filter(Boolean) : [];

    // Construct preview scenario
    const startPolicy: any = {
      mode: startLocMode,
      location_id: startLocMode === "fixed" ? startLocFixed : undefined,
      attribute_key: startLocMode === "by_attribute" ? startLocKey : undefined,
      attribute_value: startLocMode === "by_attribute" ? startLocVal : undefined,
    };

    const locationPolicy: any = {
      mode: actorLocMode,
      location_id: actorLocMode === "fixed" && actorLocFixed ? actorLocFixed : undefined,
      attribute_key: actorLocMode === "by_attribute" ? actorLocKey : undefined,
      attribute_value: actorLocMode === "by_attribute" ? actorLocVal : undefined,
    };

    const populationPolicy: any = {
      mode: actorPopMode,
      count: actorPopMode === "random" ? (actorPopCountVal ? Number(actorPopCountVal) : undefined) : undefined,
      attribute_key: actorPopMode === "by_attribute" ? actorPopKey : undefined,
      attribute_value: actorPopMode === "by_attribute" ? actorPopVal : undefined,
      location_policy: locationPolicy,
    };

    const previewScenario: any = {
      id: "preview",
      name: "Preview",
      player_start_location: startLocFixed,
      initial_qualities: {},
      actor_locations: individualActorLocations,
      storylet_ids: selectedStoryletIds,
      seed: Number.isSafeInteger(seed) && seed >= 0 ? seed : 1,
      start_location_policy: startPolicy,
      actor_population_policy: populationPolicy,
    };

    // Use workspaceProject resolution helpers
    let resolvedStartLoc = "";
    let resolvedActors: Record<string, string> = {};
    const issues: string[] = [];

    try {
      // Deterministic PRNG
      let s = (Math.floor(previewScenario.seed) >>> 0) || 1;
      const rng = () => {
        s = (s ^ (s << 13)) >>> 0;
        s = (s ^ (s >>> 17)) >>> 0;
        s = (s ^ (s << 5)) >>> 0;
        return (s >>> 0) / 4294967296;
      };

      // Resolve start location
      if (startPolicy.mode === "fixed") {
        resolvedStartLoc = startPolicy.location_id || this.cachedLocations[0]?.id || "";
      } else if (startPolicy.mode === "random") {
        if (this.cachedLocations.length) {
          const idx = Math.floor(rng() * this.cachedLocations.length);
          resolvedStartLoc = this.cachedLocations[idx].id;
        }
      } else if (startPolicy.mode === "by_attribute") {
        const matches = this.cachedLocations.filter((l: any) => {
          let val = l[startPolicy.attribute_key];
          if (val === undefined && l.environmental_params) val = l.environmental_params[startPolicy.attribute_key];
          if (val === undefined && l.devices) val = l.devices[startPolicy.attribute_key];
          return String(val ?? "").trim().toLowerCase() === String(startPolicy.attribute_value ?? "").trim().toLowerCase();
        });
        if (matches.length) {
          const idx = Math.floor(rng() * matches.length);
          resolvedStartLoc = matches[idx].id;
        } else {
          issues.push(`No location matches ${startPolicy.attribute_key} = ${startPolicy.attribute_value}`);
        }
      }

      // Resolve actors
      let candidateCharacters = [...this.cachedCharacters];
      if (populationPolicy.mode === "random") {
        const count = populationPolicy.count ?? 0;
        const pool = [...candidateCharacters];
        candidateCharacters = [];
        for (let i = 0; i < count && pool.length > 0; i++) {
          const idx = Math.floor(rng() * pool.length);
          candidateCharacters.push(pool[idx]);
          pool.splice(idx, 1);
        }
      } else if (populationPolicy.mode === "by_attribute") {
        candidateCharacters = candidateCharacters.filter((c: any) => {
          let val = c[populationPolicy.attribute_key];
          if (val === undefined && c.status_axes) val = c.status_axes[populationPolicy.attribute_key];
          if (val === undefined && c.qualities) val = c.qualities[populationPolicy.attribute_key];
          if (val === undefined && c.needs) val = c.needs[populationPolicy.attribute_key];
          return String(val ?? "").trim().toLowerCase() === String(populationPolicy.attribute_value ?? "").trim().toLowerCase();
        });
      }

      for (const char of candidateCharacters) {
        if (!locationPolicy || locationPolicy.mode === "fixed") {
          const loc = locationPolicy?.location_id || individualActorLocations[char.id] || char.current_location || char.home_location;
          resolvedActors[char.id] = loc;
        } else if (locationPolicy.mode === "random") {
          if (this.cachedLocations.length) {
            const idx = Math.floor(rng() * this.cachedLocations.length);
            resolvedActors[char.id] = this.cachedLocations[idx].id;
          }
        } else if (locationPolicy.mode === "by_attribute") {
          const matches = this.cachedLocations.filter((l: any) => {
            let val = l[locationPolicy.attribute_key];
            if (val === undefined && l.environmental_params) val = l.environmental_params[locationPolicy.attribute_key];
            if (val === undefined && l.devices) val = l.devices[locationPolicy.attribute_key];
            return String(val ?? "").trim().toLowerCase() === String(locationPolicy.attribute_value ?? "").trim().toLowerCase();
          });
          if (matches.length) {
            const idx = Math.floor(rng() * matches.length);
            resolvedActors[char.id] = matches[idx].id;
          }
        }
      }
    } catch (e) {
      console.error("Error computing preview diagnostics:", e);
    }

    // Render actors per location
    actorsPerLocContainer.replaceChildren();
    const locMap: Record<string, string[]> = {};
    for (const loc of this.cachedLocations) locMap[loc.id] = [];
    for (const [actorId, locId] of Object.entries(resolvedActors)) {
      if (!locMap[locId]) locMap[locId] = [];
      locMap[locId].push(actorId);
    }

    for (const loc of this.cachedLocations) {
      const actorsHere = locMap[loc.id] || [];
      const row = document.createElement("div");
      row.style.display = "flex";
      row.style.justifyContent = "space-between";
      row.style.padding = "2px 0";
      row.style.borderBottom = "1px solid var(--border-subtle)";
      const isStart = loc.id === resolvedStartLoc;
      row.innerHTML = `<span style="color:var(--text-main)">${loc.name}${isStart ? ' <span style="color:var(--accent-primary);font-size:10px">(Player Start)</span>' : ''}</span><span style="color:var(--text-muted);font-weight:600">${actorsHere.length} actor${actorsHere.length === 1 ? '' : 's'} ${actorsHere.length > 0 ? `(${actorsHere.join(', ')})` : ''}</span>`;
      actorsPerLocContainer.appendChild(row);
    }

    // Render storylet role availability
    roleAvailContainer.replaceChildren();
    const approvedStorylets = this.cachedStorylets.filter((s: any) => s.revision != null && s.approved_revision === s.revision);
    const displayedStorylets = selectedStoryletIds.length
      ? approvedStorylets.filter((s: any) => selectedStoryletIds.includes(s.id))
      : approvedStorylets;

    if (!displayedStorylets.length) {
      roleAvailContainer.innerHTML = '<span class="playtest-help">No approved storylets selected in scenario.</span>';
    } else {
      for (const st of displayedStorylets) {
        const item = document.createElement("div");
        item.style.padding = "4px 0";
        item.style.borderBottom = "1px solid var(--border-subtle)";

        const header = document.createElement("div");
        header.style.fontWeight = "600";
        header.style.color = "var(--text-main)";
        header.textContent = `${st.title} (${st.id})`;
        item.appendChild(header);

        // Required roles
        const reqRoles: string[] = [];
        for (const p of st.parameters ?? []) {
          const match = p.query.trim().match(/^(?:character\.)?role\s*==\s*(['"])(.*?)\1$/i);
          if (match?.[2]?.trim()) reqRoles.push(match[2].trim());
        }

        if (!reqRoles.length) {
          const noRoles = document.createElement("div");
          noRoles.style.color = "var(--text-muted)";
          noRoles.style.fontSize = "10px";
          noRoles.textContent = "No required character roles";
          item.appendChild(noRoles);
        } else {
          for (const role of reqRoles) {
            // Find candidates: must have role, and if storylet has location, actor's resolved location must match storylet location
            const candidates = Object.entries(resolvedActors).filter(([actorId, actorLoc]) => {
              const char = this.cachedCharacters.find((c: any) => c.id === actorId);
              if (!char) return false;
              const hasRole = char.role.trim().toLowerCase() === role.toLowerCase();
              if (!hasRole) return false;
              if (st.location && actorLoc !== st.location) return false;
              return true;
            }).map(([actorId]) => actorId);

            const roleRow = document.createElement("div");
            roleRow.style.display = "flex";
            roleRow.style.justifyContent = "space-between";
            roleRow.style.fontSize = "10px";
            roleRow.style.padding = "2px 0";

            const satisfied = candidates.length > 0;
            const statusColor = satisfied ? "#10b981" : "#ef4444";
            roleRow.innerHTML = `<span>Role "<strong>${role}</strong>"${st.location ? ` at ${st.location}` : ' (any loc)'}:</span><span style="color:${statusColor};font-weight:600">${candidates.length} candidate${candidates.length === 1 ? '' : 's'} ${satisfied ? `[${candidates.join(', ')}]` : '[NONE]'}</span>`;
            item.appendChild(roleRow);
          }
        }
        roleAvailContainer.appendChild(item);
      }
    }

    if (summarySpan) {
      summarySpan.textContent = issues.length
        ? `Issues: ${issues.join(', ')}`
        : `Seed ${previewScenario.seed} · ${Object.keys(resolvedActors).length} active actors`;
    }
  }

  private updatePlaytestStartButton(): void {
    const scenarioId = (document.getElementById("playtest-scenario-select") as HTMLSelectElement | null)?.value;
    const location = (document.getElementById("playtest-start-location") as HTMLSelectElement | null)?.value;
    const startLocMode = (document.getElementById("scenario-start-location-mode") as HTMLSelectElement | null)?.value;
    const button = document.getElementById("btn-start-clean-playtest") as HTMLButtonElement | null;
    if (button) button.disabled = !scenarioId && !location && startLocMode === "fixed";
  }

  private async savePlaytestScenario(): Promise<void> {
    const idInput = document.getElementById("scenario-id") as HTMLInputElement;
    const name = (document.getElementById("scenario-name") as HTMLInputElement).value.trim();
    const locationId = (document.getElementById("playtest-start-location") as HTMLSelectElement).value;
    const seed = Number((document.getElementById("scenario-seed") as HTMLInputElement).value);
    const status = document.getElementById("scenario-validation-message");
    const initialQualities: Record<string, number> = {};
    for (const input of [...(document.getElementById("scenario-initial-quality-fields")?.querySelectorAll<HTMLInputElement>("input[data-quality]") ?? [])]) {
      if (!input.value.trim()) continue;
      const value = Number(input.value);
      if (!Number.isFinite(value)) {
        if (status) status.textContent = `Starting value for ${input.dataset.quality} must be a finite number.`;
        input.focus();
        return;
      }
      initialQualities[input.dataset.quality!] = value;
    }
    const actorLocations: Record<string, string> = {};
    for (const select of [...(document.getElementById("scenario-actor-location-fields")?.querySelectorAll<HTMLSelectElement>("select[data-actor]") ?? [])]) {
      if (select.value) actorLocations[select.dataset.actor!] = select.value;
    }
    if (!idInput.value.trim()) idInput.value = name.toLowerCase().normalize("NFKD").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");
    const id = idInput.value.trim();
    const storyletIds = [...(document.getElementById("scenario-storylet-ids") as HTMLSelectElement).selectedOptions].map((option) => option.value).filter(Boolean);
    const startLocMode = (document.getElementById("scenario-start-location-mode") as HTMLSelectElement | null)?.value || "fixed";
    const startLocKey = (document.getElementById("scenario-start-location-attr-key") as HTMLSelectElement | null)?.value || "";
    const startLocVal = (document.getElementById("scenario-start-location-attr-val") as HTMLInputElement | null)?.value || "";

    const actorPopMode = (document.getElementById("scenario-actor-pop-mode") as HTMLSelectElement | null)?.value || "all";
    const actorPopCountVal = (document.getElementById("scenario-actor-pop-count") as HTMLInputElement | null)?.value;
    const actorPopKey = (document.getElementById("scenario-actor-pop-attr-key") as HTMLSelectElement | null)?.value || "";
    const actorPopVal = (document.getElementById("scenario-actor-pop-attr-val") as HTMLInputElement | null)?.value || "";

    const actorLocMode = (document.getElementById("scenario-actor-loc-mode") as HTMLSelectElement | null)?.value || "fixed";
    const actorLocFixed = (document.getElementById("scenario-actor-loc-fixed-select") as HTMLSelectElement | null)?.value || "";
    const actorLocKey = (document.getElementById("scenario-actor-loc-attr-key") as HTMLSelectElement | null)?.value || "";
    const actorLocVal = (document.getElementById("scenario-actor-loc-attr-val") as HTMLInputElement | null)?.value || "";

    const startLocationPolicy = {
      mode: startLocMode as any,
      location_id: startLocMode === "fixed" ? (locationId || undefined) : undefined,
      attribute_key: startLocMode === "by_attribute" ? (startLocKey || undefined) : undefined,
      attribute_value: startLocMode === "by_attribute" ? (startLocVal || undefined) : undefined,
    };

    const actorLocationPolicy = {
      mode: actorLocMode as any,
      location_id: actorLocMode === "fixed" && actorLocFixed ? actorLocFixed : undefined,
      attribute_key: actorLocMode === "by_attribute" ? (actorLocKey || undefined) : undefined,
      attribute_value: actorLocMode === "by_attribute" ? (actorLocVal || undefined) : undefined,
    };

    const actorPopulationPolicy = {
      mode: actorPopMode as any,
      count: actorPopMode === "random" && actorPopCountVal ? Number(actorPopCountVal) : undefined,
      attribute_key: actorPopMode === "by_attribute" ? (actorPopKey || undefined) : undefined,
      attribute_value: actorPopMode === "by_attribute" ? (actorPopVal || undefined) : undefined,
      location_policy: actorLocationPolicy,
    };

    const scenario = {
      id, name, player_start_location: locationId,
      initial_qualities: initialQualities,
      actor_locations: actorLocations,
      storylet_ids: storyletIds,
      seed,
      start_location_policy: startLocationPolicy,
      actor_population_policy: actorPopulationPolicy,
    };
    const existing = this.cachedScenarios.some((item) => item.id === id);
    const response = await fetch(existing ? `/api/v1/scenarios/${encodeURIComponent(id)}` : "/api/v1/scenarios", {
      method: existing ? "PUT" : "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(scenario),
    });
    const body = await response.json();
    if (!response.ok) {
      if (status) status.textContent = [body.error, ...(body.issues ?? [])].filter(Boolean).join(" · ");
      return;
    }
    idInput.value = body.scenario.id;
    await this.loadPlaytestScenarios();
    (document.getElementById("playtest-scenario-select") as HTMLSelectElement).value = body.scenario.id;
    this.populateScenarioEditor(body.scenario);
  }

  private async deletePlaytestScenario(): Promise<void> {
    const select = document.getElementById("playtest-scenario-select") as HTMLSelectElement;
    if (!select.value) return;
    const response = await fetch(`/api/v1/scenarios/${encodeURIComponent(select.value)}`, { method: "DELETE" });
    if (!response.ok) {
      alert("Could not delete this scenario.");
      return;
    }
    this.newPlaytestScenario();
    await this.loadPlaytestScenarios();
  }

  private async loadSavedPlaytests(): Promise<void> {
    const select = document.getElementById("playtest-saved-run") as HTMLSelectElement | null;
    const resume = document.getElementById("btn-resume-playtest") as HTMLButtonElement | null;
    if (!select) return;
    try {
      const response = await fetch("/api/v1/playtests");
      const runs = await response.json();
      if (!response.ok) throw new Error(runs.error ?? "Could not load saved runs.");
      const previous = select.value;
      select.replaceChildren();
      this.playtestBranches = runs.map((run: any) => ({
        sessionId: run.session_id,
        label: `${run.project_id} · turn ${run.turn} · ${run.event_count} events · ${run.session_id.slice(0, 8)}`,
      }));
      if (!runs.length) {
        const empty = document.createElement("option");
        empty.value = "";
        empty.textContent = "No saved runs";
        select.appendChild(empty);
      } else {
        for (const run of runs) {
          const option = document.createElement("option");
          option.value = run.session_id;
          option.textContent = `${run.project_id} · turn ${run.turn} · ${run.event_count} events · ${run.session_id.slice(0, 8)}`;
          select.appendChild(option);
        }
        select.value = runs.some((run: any) => run.session_id === previous)
          ? previous
          : this.playtestSessionId && runs.some((run: any) => run.session_id === this.playtestSessionId)
            ? this.playtestSessionId : runs[0].session_id;
      }
      if (resume) resume.disabled = !runs.length;
      this.renderBranchControls();
    } catch (error) {
      console.error("Could not load saved playtests:", error);
    }
  }

  private async resumeSavedPlaytest(): Promise<void> {
    const select = document.getElementById("playtest-saved-run") as HTMLSelectElement | null;
    if (!select?.value) return;
    try {
      const response = await fetch(`/api/v1/playtests/${encodeURIComponent(select.value)}`);
      const snapshot = await response.json();
      if (!response.ok) throw new Error(snapshot.error ?? "Could not resume this run.");
      this.playtestSessionId = snapshot.session_id;
      this.playtestSnapshot = snapshot;
      this.playtestDecisionOverride = null;
      this.selectedButterflyEventId = null;
      this.renderPlaytestSnapshot();
      await this.refreshPlaytestDirector();
    } catch (error) {
      alert(error instanceof Error ? error.message : String(error));
    }
  }

  private populatePlaytestStartLocations(locations: any[]): void {
    this.cachedLocations = locations;
    const select = document.getElementById("playtest-start-location") as HTMLSelectElement | null;
    const startButton = document.getElementById("btn-start-clean-playtest") as HTMLButtonElement | null;
    if (!select) return;
    const previous = select.value;
    select.replaceChildren(new Option(locations.length ? "Choose a location" : "Add a location in Codex", ""));
    for (const location of locations) {
      select.add(new Option(location.name, location.id));
    }
    if (locations.some((location) => location.id === previous)) select.value = previous;
    else if (locations.length === 1) select.value = locations[0].id;
    if (startButton) startButton.disabled = locations.length === 0 || !select.value;
    select.onchange = () => {
      if (startButton) startButton.disabled = locations.length === 0 || !select.value;
    };
  }

  private async startCleanPlaytest(): Promise<void> {
    const button = document.getElementById("btn-start-clean-playtest") as HTMLButtonElement;
    const scenarioSelect = document.getElementById("playtest-scenario-select") as HTMLSelectElement | null;
    const scenarioId = scenarioSelect?.value;
    const startLocationId = (document.getElementById("playtest-start-location") as HTMLSelectElement).value;
    if (!scenarioId && !startLocationId) return;
    button.disabled = true;
    button.innerText = "Building clean scenario…";
    try {
      const response = await fetch("/api/v1/playtests/from-workspace", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify((document.getElementById("playtest-scenario-select") as HTMLSelectElement).value
          ? { scenarioId: (document.getElementById("playtest-scenario-select") as HTMLSelectElement).value }
          : { startLocationId }),
      });
      const body = await response.json();
      if (!response.ok) {
        const detail = Array.isArray(body.issues) ? body.issues.join("\n") : body.error;
        throw new Error(detail ?? "Could not start playtest.");
      }
      this.playtestSessionId = body.sessionId;
      this.playtestSnapshot = body.snapshot;
      this.playtestDecisionOverride = null;
      this.selectedButterflyEventId = null;
      await this.loadSavedPlaytests();
      this.renderPlaytestSnapshot();
      await this.refreshPlaytestDirector();
    } catch (error) {
      alert(error instanceof Error ? error.message : String(error));
    } finally {
      button.disabled = false;
      button.innerText = this.playtestSessionId ? "Restart from clean state" : "Start clean playtest";
    }
  }

  private async refreshPlaytest(): Promise<void> {
    if (!this.playtestSessionId) return;
    try {
      const response = await fetch(`/api/v1/playtests/${encodeURIComponent(this.playtestSessionId)}`);
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "Could not refresh playtest.");
      this.playtestSnapshot = body;
      this.renderPlaytestSnapshot();
      await this.refreshPlaytestDirector();
    } catch (error) {
      console.error("Failed to refresh playtest:", error);
    }
  }

  private renderPlaytestSnapshot(): void {
    if (!this.playtestSnapshot) return;
    const { world, session_id: sessionId } = this.playtestSnapshot;
    this.connectPlaytestEvents(sessionId);
    const state = document.getElementById("playtest-session-state");
    const workspace = document.getElementById("playtest-session-workspace");
    const location = this.cachedLocations.find((item) => item.id === world.current_location);
    if (state) state.innerHTML = `<div class="playtest-session-status">Run ${sessionId} · Turn ${world.turn} · Revision ${world.revision}</div>`;
    workspace?.classList.remove("hidden");
    const turn = document.getElementById("playtest-turn-badge");
    const locationTitle = document.getElementById("playtest-location-title");
    if (turn) turn.textContent = `Turn ${world.turn}`;
    if (locationTitle) locationTitle.textContent = location?.name ?? world.current_location ?? "No location";

    const qualityList = document.getElementById("playtest-quality-list");
    qualityList?.replaceChildren();
    for (const [key, value] of Object.entries(world.qualities ?? {})) {
      const chip = document.createElement("span");
      chip.className = "playtest-quality-chip";
      chip.textContent = `${key}: ${String(value)}`;
      qualityList?.appendChild(chip);
    }
    if (qualityList && !qualityList.childElementCount) {
      qualityList.textContent = "This scenario has no qualities yet.";
      qualityList.className = "playtest-quality-list playtest-list-empty";
    } else if (qualityList) {
      qualityList.className = "playtest-quality-list";
    }

    const eventFeed = document.getElementById("playtest-event-feed");
    eventFeed?.replaceChildren();
    const events = world.events ?? [];
    const count = document.getElementById("playtest-event-count");
    if (count) count.textContent = `${events.length} ${events.length === 1 ? "event" : "events"}`;
    for (const event of events.slice().reverse()) {
      const item = document.createElement("article");
      item.className = "playtest-event-item";
      const title = document.createElement("strong");
      title.textContent = `Turn ${event.turn} · ${event.actor_id} · ${event.action}`;
      const details = document.createElement("span");
      const deltas = (event.deltas ?? []).map((delta: any) => `${delta.key}: ${JSON.stringify(delta.before)} → ${JSON.stringify(delta.after)}`);
      details.textContent = [deltas.join(" · "), event.caused_by?.length ? `Caused by ${event.caused_by.join(", ")}` : ""].filter(Boolean).join(" | ") || "No quality changes";
      item.append(title, details);
      eventFeed?.appendChild(item);
    }
    if (eventFeed && !eventFeed.childElementCount) {
      const empty = document.createElement("p");
      empty.className = "playtest-list-empty";
      empty.textContent = "Actions and consequences will appear here.";
      eventFeed.appendChild(empty);
    }

    const waitButton = document.getElementById("btn-playtest-wait") as HTMLButtonElement | null;
    if (waitButton) waitButton.disabled = false;
    this.renderPlaytestRoutes(world.current_location);
    this.refreshButterflyChain();
    this.renderBranchControls();
  }

  private connectPlaytestEvents(sessionId: string): void {
    if (this.playtestEventsSessionId === sessionId) return;
    this.playtestEvents?.close();
    this.playtestEventsSessionId = sessionId;
    const source = new EventSource(`/api/v1/playtests/${encodeURIComponent(sessionId)}/stream`);
    this.playtestEvents = source;
    const applySnapshot = (event: MessageEvent<string>) => {
      if (this.playtestSessionId !== sessionId) return;
      try {
        const message = JSON.parse(event.data);
        const snapshot = message.snapshot;
        if (!snapshot || snapshot.session_id !== sessionId) return;
        const currentRevision = this.playtestSnapshot?.world?.revision ?? -1;
        if (snapshot.world.revision <= currentRevision) return;
        this.playtestSnapshot = snapshot;
        this.renderPlaytestSnapshot();
        void this.refreshPlaytestDirector();
      } catch (error) {
        console.error("Could not apply live playtest update:", error);
      }
    };
    source.addEventListener("playtest.snapshot", applySnapshot as EventListener);
    source.addEventListener("playtest.updated", applySnapshot as EventListener);
    source.addEventListener("playtest.resync", () => { void this.refreshPlaytest(); });
    source.onerror = () => console.warn(`Playtest event stream disconnected for ${sessionId}; EventSource will retry.`);
  }

  private renderBranchControls(): void {
    const selected = document.getElementById("playtest-selected-event");
    const forkButton = document.getElementById("btn-playtest-fork-event") as HTMLButtonElement | null;
    const select = document.getElementById("playtest-compare-run") as HTMLSelectElement | null;
    const compareButton = document.getElementById("btn-playtest-compare") as HTMLButtonElement | null;
    const event = (this.playtestSnapshot?.world?.events ?? []).find((item: any) => item.id === this.selectedButterflyEventId);
    if (selected) selected.textContent = event ? `Selected: turn ${event.turn} · ${event.actor_id} · ${event.action}` : "Select an event to create a branch.";
    if (forkButton) forkButton.disabled = !this.playtestSessionId || !event;
    if (!select) return;
    const previous = select.value;
    select.replaceChildren();
    const others = this.playtestBranches.filter((branch) => branch.sessionId !== this.playtestSessionId);
    if (!others.length) {
      const option = document.createElement("option");
      option.value = "";
      option.textContent = "No branch to compare";
      select.appendChild(option);
    } else {
      for (const branch of others) {
        const option = document.createElement("option");
        option.value = branch.sessionId;
        option.textContent = branch.label;
        select.appendChild(option);
      }
      select.value = others.some((branch) => branch.sessionId === previous) ? previous : others[0].sessionId;
    }
    select.disabled = !this.playtestSessionId || !others.length;
    if (compareButton) compareButton.disabled = !this.playtestSessionId || !others.length;
  }

  private async forkPlaytestAtSelectedEvent(): Promise<void> {
    if (!this.playtestSessionId || !this.selectedButterflyEventId) return;
    const sourceSessionId = this.playtestSessionId;
    const response = await fetch(`/api/v1/playtests/${encodeURIComponent(sourceSessionId)}/fork`, {
      method: "POST", headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ branchSessionId: crypto.randomUUID(), eventId: this.selectedButterflyEventId }),
    });
    const body = await response.json();
    if (!response.ok) {
      alert(body.error ?? "Could not replay this event prefix.");
      return;
    }
    if (!this.playtestBranches.some((branch) => branch.sessionId === sourceSessionId)) {
      this.playtestBranches.push({ sessionId: sourceSessionId, label: `Run ${sourceSessionId.slice(0, 8)}` });
    }
    this.playtestSessionId = body.sessionId;
    this.playtestSnapshot = body.snapshot;
      this.playtestDecisionOverride = null;
    this.selectedButterflyEventId = null;
    this.playtestBranches.push({ sessionId: body.sessionId, label: `Branch ${body.sessionId.slice(0, 8)} · turn ${body.snapshot.world.turn}` });
    this.renderPlaytestSnapshot();
    this.renderBranchControls();
    await this.refreshPlaytestDirector();
  }

  private async comparePlaytestBranches(): Promise<void> {
    const select = document.getElementById("playtest-compare-run") as HTMLSelectElement | null;
    const container = document.getElementById("playtest-branch-comparison");
    if (!select?.value || !this.playtestSessionId || !container) return;
    try {
      const [activeResponse, otherResponse] = await Promise.all([
        fetch(`/api/v1/playtests/${encodeURIComponent(this.playtestSessionId)}`),
        fetch(`/api/v1/playtests/${encodeURIComponent(select.value)}`),
      ]);
      const [active, other] = await Promise.all([activeResponse.json(), otherResponse.json()]);
      if (!activeResponse.ok || !otherResponse.ok) throw new Error(active.error ?? other.error ?? "Could not load both runs.");
      const activeEvents = active.world.events ?? [];
      const otherEvents = other.world.events ?? [];
      const otherIds = new Set(otherEvents.map((item: any) => item.id));
      const activeIds = new Set(activeEvents.map((item: any) => item.id));
      const activeOnly = activeEvents.filter((item: any) => !otherIds.has(item.id));
      const otherOnly = otherEvents.filter((item: any) => !activeIds.has(item.id));
      const keys = new Set([...Object.keys(active.world.qualities ?? {}), ...Object.keys(other.world.qualities ?? {})]);
      const differences = [...keys].filter((key) => JSON.stringify(active.world.qualities?.[key]) !== JSON.stringify(other.world.qualities?.[key]))
        .map((key) => `${key}: ${JSON.stringify(other.world.qualities?.[key])} → ${JSON.stringify(active.world.qualities?.[key])}`);
      const eventLabels = (events: any[]) => events.slice(0, 3).map((item) => `turn ${item.turn} ${item.actor_id}: ${item.action}`).join("; ");
      container.textContent = `Active turn ${active.world.turn} vs comparison turn ${other.world.turn}. Active-only (${activeOnly.length}): ${eventLabels(activeOnly) || "none"}. Comparison-only (${otherOnly.length}): ${eventLabels(otherOnly) || "none"}. ${differences.length ? differences.join(" · ") : "Quality state currently matches."}`;
    } catch (error) {
      container.textContent = error instanceof Error ? error.message : String(error);
    }
  }

  private renderPlaytestRoutes(locationId: string | null): void {
    const container = document.getElementById("playtest-location-actions");
    if (!container) return;
    container.replaceChildren();
    const current = this.cachedLocations.find((item) => item.id === locationId);
    const connected = new Set(current?.connected_zones ?? []);
    const routes = this.cachedLocations.filter((location) => connected.has(location.id));
    if (!routes.length) {
      const empty = document.createElement("p");
      empty.className = "playtest-list-empty";
      empty.textContent = "No connected routes. Add location connections in the Codex to move between places.";
      container.appendChild(empty);
      return;
    }
    for (const route of routes) {
      const button = document.createElement("button");
      button.type = "button";
      button.dataset.locationId = route.id;
      button.textContent = `Move to ${route.name}`;
      container.appendChild(button);
    }
  }

  private async submitPlaytestAction(action: string, effects: Array<Record<string, unknown>>): Promise<void> {
    if (!this.playtestSessionId || !this.playtestSnapshot) return;
    const waitButton = document.getElementById("btn-playtest-wait") as HTMLButtonElement | null;
    if (waitButton) waitButton.disabled = true;
    try {
      const response = await fetch(`/api/v1/playtests/${encodeURIComponent(this.playtestSessionId)}/commands`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          command: {
            idempotency_key: `web:${crypto.randomUUID()}`,
            expected_revision: this.playtestSnapshot.world.revision,
            actor_id: "player",
            action,
            effects,
            caused_by: [],
            advance_turn: true,
          },
        }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "The action could not be applied.");
      this.playtestSnapshot.world = body.world;
      this.renderPlaytestSnapshot();
      await this.loadSavedPlaytests();
      await this.refreshPlaytestDirector();
    } catch (error) {
      alert(error instanceof Error ? error.message : String(error));
      await this.refreshPlaytest();
    } finally {
      if (waitButton) waitButton.disabled = false;
    }
  }

  private async refreshPlaytestDirector(): Promise<void> {
    const rankButton = document.getElementById("btn-rank-local-encoder") as HTMLButtonElement | null;
    if (rankButton) rankButton.disabled = !this.playtestSessionId;
    if (!this.playtestSessionId) return;
    try {
      const response = await fetch(`/api/v1/playtests/${encodeURIComponent(this.playtestSessionId)}/director`);
      const responseEvaluation = await response.json();
      if (!response.ok) throw new Error(responseEvaluation.error ?? "Could not evaluate storylets.");
      const evaluation = this.playtestDecisionOverride?.decision?.contextRevision === this.playtestSnapshot?.world?.revision
        ? this.playtestDecisionOverride
        : responseEvaluation;
      if (evaluation !== this.playtestDecisionOverride) this.playtestDecisionOverride = null;
      const container = document.getElementById("playtest-candidate-list");
      if (!container) return;
      container.replaceChildren();
      const candidates = evaluation.candidates ?? [];
      const selectedId = evaluation.selected as string | null;
      const selectedStorylet = this.cachedStorylets.find((storylet) => storylet.id === selectedId);
      const sceneLabel = document.getElementById("playtest-scene-label");
      const sceneText = document.getElementById("playtest-scene-text");
      const choiceList = document.getElementById("playtest-choice-list");
      if (sceneLabel) sceneLabel.textContent = selectedStorylet?.title
        ?? (evaluation.decision?.source === "embedding_baseline_uncalibrated" ? "Director deferred scene" : "No eligible scene");
      if (sceneText) sceneText.textContent = selectedStorylet?.content?.sceneOpening
        ?? selectedStorylet?.content?.text_template
        ?? (evaluation.decision?.source === "embedding_baseline_uncalibrated"
          ? "The local semantic baseline deferred this scene. Review the eligible storylets and continue the playtest."
          : "The director will surface scenes when their approved triggers and roles are available.");
      choiceList?.replaceChildren();
      if (selectedStorylet && choiceList) {
        for (const choice of selectedStorylet.choices ?? []) {
          const button = document.createElement("button");
          button.type = "button";
          button.dataset.storyletId = selectedStorylet.id;
          button.dataset.choiceId = choice.choice_id;
          button.textContent = choice.label;
          choiceList.appendChild(button);
        }
      }
      for (const candidate of candidates) {
        const item = document.createElement("article");
        item.className = "playtest-event-item";
        const title = document.createElement("strong");
        title.textContent = `${candidate.storylet_id}${candidate.storylet_id === evaluation.selected ? " · selected" : ""}`;
        const reason = document.createElement("span");
        const ranked = evaluation.decision?.ranking?.find((item: any) => item.storyletId === candidate.storylet_id);
        reason.textContent = candidate.eligible
          ? `Eligible · weight ${Number(candidate.final_weight).toFixed(1)}${ranked ? ` · semantic rank ${(ranked.probability * 100).toFixed(1)}%` : ""}`
          : `Rejected · ${candidate.rejection?.reason ?? "ineligible"}`;
        item.append(title, reason);
        container.appendChild(item);
      }
      if (!candidates.length) {
        container.className = "playtest-list-empty";
        container.textContent = "No storylets are in this scenario. Add storylets in the Workshop.";
      } else {
        container.className = "playtest-event-feed";
      }
      const decisionNotice = document.getElementById("playtest-director-notice");
      if (decisionNotice) {
        decisionNotice.textContent = evaluation.decision?.source === "embedding_baseline_uncalibrated"
          ? `Uncalibrated semantic baseline · ${evaluation.decision.modelVersion}. Probabilities are provisional, not trained confidence.`
          : "Rust deterministic baseline. Use the local encoder to compare semantic relevance.";
      }
    } catch (error) {
      console.error("Failed to evaluate Rust playtest director:", error);
    }
  }

  private async rankPlaytestWithLocalEncoder(): Promise<void> {
    if (!this.playtestSessionId) return;
    const button = document.getElementById("btn-rank-local-encoder") as HTMLButtonElement | null;
    if (button) button.disabled = true;
    try {
      const response = await fetch(`/api/v1/playtests/${encodeURIComponent(this.playtestSessionId)}/director/rank`, { method: "POST" });
      const result = await response.json();
      if (!response.ok) throw new Error(result.error ?? "Local semantic ranking failed.");
      this.playtestDecisionOverride = result;
      await this.refreshPlaytestDirector();
      const status = document.getElementById("playtest-scene-label");
      if (status && result.decision?.source === "embedding_baseline_uncalibrated") {
        status.title = `Ranked with ${result.decision.modelVersion}; scores are an uncalibrated baseline.`;
      }
    } catch (error) {
      alert(error instanceof Error
        ? `${error.message}\n\nStart the optional local embedding service and try again.`
        : String(error));
    } finally {
      if (button) button.disabled = false;
    }
  }

  private async submitPlaytestChoice(storyletId: string, choiceId: string, button: HTMLButtonElement): Promise<void> {
    if (!this.playtestSessionId || !this.playtestSnapshot) return;
    const choiceButtons = document.querySelectorAll<HTMLButtonElement>("#playtest-choice-list button");
    choiceButtons.forEach((item) => item.disabled = true);
    try {
      const response = await fetch(`/api/v1/playtests/${encodeURIComponent(this.playtestSessionId)}/choices`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          storyletId,
          choiceId,
          idempotencyKey: `web-choice:${crypto.randomUUID()}`,
          expectedRevision: this.playtestSnapshot.world.revision,
        }),
      });
      const body = await response.json();
      if (!response.ok) throw new Error(body.error ?? "The storylet choice could not be applied.");
      this.playtestSnapshot.world = body.world;
      this.renderPlaytestSnapshot();
      await this.loadSavedPlaytests();
      await this.refreshPlaytestDirector();
    } catch (error) {
      alert(error instanceof Error ? error.message : String(error));
      await this.refreshPlaytest();
    } finally {
      button.disabled = false;
    }
  }
}
