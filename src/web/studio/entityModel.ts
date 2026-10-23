import type {
  CharacterRecord,
  CharacterSection,
  EntityRecord,
  EntityType,
  LocationRecord,
  LocationSection,
  TraitTemplateRecord,
} from "./contracts.ts";

export interface EntityTypeDefinition {
  value: EntityType;
  label: string;
  color: string;
}

export const ENTITY_TYPES: EntityTypeDefinition[] = [
  { value: "character", label: "Character", color: "var(--type-character)" },
  { value: "location", label: "Location", color: "var(--type-location)" },
  { value: "item", label: "Item", color: "var(--type-item)" },
  { value: "fact", label: "Fact", color: "var(--type-fact)" },
  { value: "other", label: "Other", color: "var(--type-other)" },
];

export function entityTypeLabel(type: EntityType | string): string {
  return ENTITY_TYPES.find((entry) => entry.value === type)?.label ?? "Other";
}

export function entityTypeColor(type: EntityType | string): string {
  return ENTITY_TYPES.find((entry) => entry.value === type)?.color ?? "var(--type-other)";
}

export function defaultCharacterSection(): CharacterSection {
  return {
    role: "",
    home_location: "",
    faction_id: undefined,
    current_location: "",
    status_axes: {},
    needs: {},
    agenda: "",
    red_line: "",
    schedule: undefined,
    qualities: undefined,
    passion_pool: [],
    active_passions: [],
    trust_inertia: 0,
    thresholds: [],
    zero_sum_passion_links: [],
  };
}

export function defaultLocationSection(): LocationSection {
  return {
    tags: [],
    connected_zones: [],
    environmental_params: {},
    devices: {},
  };
}

/** Switch an entity's kind, keeping only the section that kind owns. */
export function withEntityType(entity: EntityRecord, type: EntityType): EntityRecord {
  const next: EntityRecord = { ...entity, type };
  if (type === "character") {
    next.character = entity.character ?? defaultCharacterSection();
    delete next.location;
  } else if (type === "location") {
    next.location = entity.location ?? defaultLocationSection();
    delete next.character;
  } else {
    delete next.character;
    delete next.location;
  }
  return next;
}

export function characterRecordFromEntity(entity: EntityRecord): CharacterRecord | null {
  if (entity.type !== "character" || !entity.character) {
    return null;
  }
  return {
    id: entity.id,
    name: entity.name,
    ...entity.character,
    created_at: entity.created_at,
  };
}

export function locationRecordFromEntity(entity: EntityRecord): LocationRecord | null {
  if (entity.type !== "location" || !entity.location) {
    return null;
  }
  return {
    id: entity.id,
    name: entity.name,
    description: entity.description ?? "",
    ...entity.location,
    created_at: entity.created_at,
  };
}

export function newEntityDraft(folderId: string | null): EntityRecord {
  return {
    id: "",
    object_id: "",
    game_id: "",
    name: "New Entity",
    type: "character",
    description: "",
    thumbnail: null,
    folder_id: folderId,
    properties: [],
    traits: [],
    character: defaultCharacterSection(),
  };
}

/** Applied templates, in the entity's own order. */
export function appliedTemplates(
  entity: EntityRecord,
  templates: TraitTemplateRecord[],
): TraitTemplateRecord[] {
  return entity.traits
    .map((instance) => templates.find((template) => template.id === instance.template_id))
    .filter((template): template is TraitTemplateRecord => template !== undefined);
}
