export interface NavItem {
  id: string;
  label: string;
  desc: string;
  icon: string;
  path: string;
}

export const NAVIGATION_ITEMS: NavItem[] = [
  { id: "storylets", label: "Storylets", desc: "Library & editor", icon: "•", path: "/storylets" },
  { id: "codex", label: "Story Codex", desc: "World records & events", icon: "•", path: "/codex" },
  { id: "scenarios", label: "Starting Scenarios", desc: "Initial state presets", icon: "•", path: "/scenarios" },
  { id: "generation", label: "Strata Generation", desc: "Staged scene drafting", icon: "•", path: "/generation" },
  { id: "playtest", label: "Playtest & Director", desc: "Interactive runtime", icon: "•", path: "/playtest" },
  { id: "project", label: "Project & Export", desc: "Metadata & bundles", icon: "•", path: "/project" },
];

export const CODEX_CATEGORIES = [
  { id: "qualities", label: "Qualities", entity: "qualities" },
  { id: "characters", label: "Characters", entity: "characters" },
  { id: "locations", label: "Locations", entity: "locations" },
  { id: "npc_behaviors", label: "NPC Rules", entity: "npc_behaviors" },
  { id: "events", label: "Project Events", entity: "events" },
] as const;
