export interface TreeExample {
  id: string;
  name: string;
  description: string;
  items: { key: string; name: string }[];
  edges: { from: string; to: string }[];
  direction: 'right' | 'left' | 'down' | 'up' | 'radial';
}

export const TREE_EXAMPLES: readonly TreeExample[] = [
  {
    id: "skill-tree",
    name: "Skill tree — Elemental Masteries",
    description: "Radial skill progression tree with a single root branching into element schools and merging into advanced hybrid disciplines.",
    direction: "radial",
    items: [
      { key: "arcane_core", name: "Arcane Core" },
      { key: "pyromancy", name: "Pyromancy" },
      { key: "cryomancy", name: "Cryomancy" },
      { key: "electromancy", name: "Electromancy" },
      { key: "plasma_surge", name: "Plasma Surge" },
      { key: "frostfire", name: "Frostfire Synthesis" },
      { key: "superconductor", name: "Superconductor" },
      { key: "arcane_singularity", name: "Arcane Singularity" },
    ],
    edges: [
      { from: "arcane_core", to: "pyromancy" },
      { from: "arcane_core", to: "cryomancy" },
      { from: "arcane_core", to: "electromancy" },
      { from: "pyromancy", to: "plasma_surge" },
      { from: "electromancy", to: "plasma_surge" },
      { from: "pyromancy", to: "frostfire" },
      { from: "cryomancy", to: "frostfire" },
      { from: "cryomancy", to: "superconductor" },
      { from: "electromancy", to: "superconductor" },
      { from: "plasma_surge", to: "arcane_singularity" },
      { from: "frostfire", to: "arcane_singularity" },
      { from: "superconductor", to: "arcane_singularity" },
    ],
  },
  {
    id: "roguelike-map",
    name: "Roguelike map — Dungeon Descent",
    description: "Branching and converging dungeon encounter graph starting from two separate entry gates descending down to the boss sanctum.",
    direction: "down",
    items: [
      { key: "gate_catacombs", name: "Catacombs Gate" },
      { key: "gate_overlook", name: "Overlook Gate" },
      { key: "chamber_goblins", name: "Goblin Outpost" },
      { key: "chamber_crossroads", name: "Crossroads Shrine" },
      { key: "chamber_beasts", name: "Beast Den" },
      { key: "chamber_merchant", name: "Wandering Merchant" },
      { key: "chamber_vault", name: "Forgotten Vault" },
      { key: "sanctuary_rest", name: "Hallowed Rest" },
      { key: "sanctum_boss", name: "Dragon Sanctum" },
    ],
    edges: [
      { from: "gate_catacombs", to: "chamber_goblins" },
      { from: "gate_catacombs", to: "chamber_crossroads" },
      { from: "gate_overlook", to: "chamber_crossroads" },
      { from: "gate_overlook", to: "chamber_beasts" },
      { from: "chamber_goblins", to: "chamber_merchant" },
      { from: "chamber_crossroads", to: "chamber_merchant" },
      { from: "chamber_crossroads", to: "chamber_vault" },
      { from: "chamber_beasts", to: "chamber_vault" },
      { from: "chamber_merchant", to: "sanctuary_rest" },
      { from: "chamber_vault", to: "sanctuary_rest" },
      { from: "sanctuary_rest", to: "sanctum_boss" },
    ],
  },
  {
    id: "relationship-tree",
    name: "Relationship tree — Court Intrigue",
    description: "Multi-root faction alliance and relationship web tracking shared loyalties and influence spreading across the court.",
    direction: "right",
    items: [
      { key: "queen_regent", name: "Queen Regent Valia" },
      { key: "grand_inquisitor", name: "Grand Inquisitor Corvus" },
      { key: "guild_master", name: "Guild Master Thorne" },
      { key: "general", name: "General Valeria" },
      { key: "envoy", name: "Diplomatic Envoy Jesse" },
      { key: "spymaster", name: "Master of Whispers Morvath" },
      { key: "crown_prince", name: "Crown Prince Corin" },
    ],
    edges: [
      { from: "queen_regent", to: "general" },
      { from: "queen_regent", to: "envoy" },
      { from: "queen_regent", to: "crown_prince" },
      { from: "grand_inquisitor", to: "general" },
      { from: "grand_inquisitor", to: "spymaster" },
      { from: "guild_master", to: "envoy" },
      { from: "guild_master", to: "spymaster" },
      { from: "envoy", to: "spymaster" },
      { from: "general", to: "crown_prince" },
    ],
  },
];
