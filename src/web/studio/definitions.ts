export interface NavItem {
  id: string;
  label: string;
  desc: string;
  icon: string;
  path: string;
}

export const NAVIGATION_ITEMS: NavItem[] = [
  { id: "tags", label: "Tag manager", desc: "Tag usage & references", icon: "•", path: "/tags" },
  { id: "codex", label: "Rules & events", desc: "Project events", icon: "•", path: "/codex" },
  { id: "plugins", label: "Plugins", desc: "Enable and inspect plugins", icon: "•", path: "/plugins" },
  { id: "project", label: "Project & export", desc: "Metadata & bundles", icon: "•", path: "/project" },
];

export const CODEX_CATEGORIES = [
  { id: "events", label: "Project Events", entity: "events" },
] as const;
