import type {
  AssetLinkValue,
  EntityProperty,
  PropertyDefinition,
  PropertyScalar,
  PropertyType,
  PropertyValue,
  TraitInstance,
} from "./contracts.ts";

export interface PropertyTypeDefinition {
  value: PropertyType;
  label: string;
}

export const PROPERTY_TYPES: PropertyTypeDefinition[] = [
  { value: "bool", label: "Boolean" },
  { value: "string", label: "String" },
  { value: "float", label: "Float" },
  { value: "int", label: "Integer" },
  { value: "asset", label: "Asset Link" },
  { value: "entity", label: "Entity Reference" },
];

export function propertyTypeLabel(type: PropertyType | string): string {
  return PROPERTY_TYPES.find((entry) => entry.value === type)?.label ?? "String";
}

function isAssetLink(value: unknown): value is AssetLinkValue {
  if (!value || typeof value !== "object") return false;
  const link = value as Partial<AssetLinkValue>;
  return typeof link.asset_id === "string" && typeof link.version === "string";
}

function scalarMatches(type: PropertyType, value: PropertyScalar): boolean {
  switch (type) {
    case "bool":
      return typeof value === "boolean";
    case "string":
      return typeof value === "string";
    case "float":
      return typeof value === "number" && Number.isFinite(value);
    case "int":
      return typeof value === "number" && Number.isInteger(value);
    case "asset":
      return value === null || isAssetLink(value);
    case "entity":
      return value === null || typeof value === "string";
    default:
      return false;
  }
}

export function defaultPropertyValue(type: PropertyType, isArray: boolean): PropertyValue {
  if (isArray) return [];
  switch (type) {
    case "bool":
      return false;
    case "string":
      return "";
    case "float":
      return 0;
    case "int":
      return 0;
    case "asset":
    case "entity":
      return null;
    default:
      return "";
  }
}

export function isValidPropertyValue(type: PropertyType, isArray: boolean, value: PropertyValue): boolean {
  if (isArray) {
    if (!Array.isArray(value)) return false;
    return value.every((item) => item !== null && scalarMatches(type, item));
  }
  if (Array.isArray(value)) return false;
  return scalarMatches(type, value);
}

/** Keep the current value when it still fits the new shape, otherwise default it. */
export function retypeValue(type: PropertyType, isArray: boolean, value: PropertyValue): PropertyValue {
  return isValidPropertyValue(type, isArray, value) ? value : defaultPropertyValue(type, isArray);
}

export function toggleArray(
  value: PropertyValue,
  toArray: boolean,
  type: PropertyType,
): PropertyValue {
  if (toArray) return [];
  if (!Array.isArray(value)) return value;
  const first = value.find((item) => item !== null && scalarMatches(type, item));
  return first ?? defaultPropertyValue(type, false);
}

export function isValidPropertyKey(key: string): boolean {
  return /^[A-Za-z_][A-Za-z0-9_]*$/.test(key.trim());
}

export function uniquePropertyKey(existing: string[], base = "NewVar"): string {
  const taken = new Set(existing.map((key) => key.toLowerCase()));
  if (!taken.has(base.toLowerCase())) return base;
  let index = 1;
  while (taken.has(`${base}_${index}`.toLowerCase())) {
    index += 1;
  }
  return `${base}_${index}`;
}

/** First authoring problem in a property list, or null when the list can be saved. */
export function propertyListProblem(
  rows: Array<{ key: string; name: string }>,
): string | null {
  const seen = new Set<string>();
  for (const row of rows) {
    const key = row.key.trim();
    if (!isValidPropertyKey(key)) return "Property keys must be identifiers";
    if (seen.has(key)) return `Duplicate property key '${key}'`;
    seen.add(key);
    if (!row.name.trim()) return `Property '${key}' needs a name`;
  }
  return null;
}

function newIdentifier(): string {
  const cryptoApi = globalThis.crypto;
  if (cryptoApi && typeof cryptoApi.randomUUID === "function") {
    return cryptoApi.randomUUID();
  }
  return `id-${Math.random().toString(36).slice(2)}-${Date.now().toString(36)}`;
}

export function newEntityProperty(existing: EntityProperty[]): EntityProperty {
  return {
    id: newIdentifier(),
    key: uniquePropertyKey(existing.map((property) => property.key)),
    name: "New Property",
    type: "bool",
    is_array: false,
    multiline: false,
    value: false,
  };
}

export function newPropertyDefinition(existing: PropertyDefinition[]): PropertyDefinition {
  return {
    id: newIdentifier(),
    key: uniquePropertyKey(existing.map((definition) => definition.key)),
    name: "New Property",
    type: "bool",
    is_array: false,
    multiline: false,
    default: false,
  };
}

/** The override when it still fits the definition, otherwise the template default. */
export function effectiveTraitValue(
  definition: PropertyDefinition,
  instance: TraitInstance | undefined,
): PropertyValue {
  const override = instance?.values?.[definition.id];
  if (override !== undefined && isValidPropertyValue(definition.type, definition.is_array ?? false, override)) {
    return override;
  }
  return definition.default ?? defaultPropertyValue(definition.type, definition.is_array ?? false);
}

export function hasTraitOverride(instance: TraitInstance | undefined, definitionId: string): boolean {
  return instance?.values?.[definitionId] !== undefined;
}
