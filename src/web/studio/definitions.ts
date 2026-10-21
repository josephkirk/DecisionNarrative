export interface NavItem {
  id: string;
  label: string;
  desc: string;
  icon: string;
  path: string;
}

export const NAVIGATION_ITEMS: NavItem[] = [
  { id: "storylets", label: "Storylet Library", desc: "Library & editor", icon: "•", path: "/storylets" },
  { id: "tags", label: "Tag Manager", desc: "Tag usage & references", icon: "•", path: "/tags" },
  { id: "codex", label: "Rules & Events", desc: "NPC rules & world events", icon: "•", path: "/codex" },
  { id: "scenarios", label: "Scenarios", desc: "Initial state presets", icon: "•", path: "/scenarios" },
  { id: "playtest", label: "Playtest", desc: "Interactive runtime", icon: "•", path: "/playtest" },
  { id: "project", label: "Project & Export", desc: "Metadata & bundles", icon: "•", path: "/project" },
];

export const CODEX_CATEGORIES = [
  { id: "npc_behaviors", label: "NPC Rules", entity: "npc_behaviors" },
  { id: "events", label: "Project Events", entity: "events" },
] as const;
