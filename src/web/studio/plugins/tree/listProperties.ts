import type {
  TraitPropertyDefinition,
  PropertyValue,
  TraitInstance,
  TraitTemplateRecord,
} from "../../contracts.ts";
import { defaultPropertyValue } from "../../propertyModel.ts";
import type { EntityRecord } from "../../contracts.ts";

export interface TreeListColumn {
  id: string;
  label: string;
  property: TraitPropertyDefinition;
  traitTemplateId: string;
}

export function treeItemColumns(template: TraitTemplateRecord | undefined): TreeListColumn[] {
  if (!template || !Array.isArray(template.properties)) {
    return [];
  }
  return template.properties.map((prop) => ({
    id: prop.key,
    label: prop.name || prop.key,
    property: prop,
    traitTemplateId: template.id,
  }));
}

export function treeColumnValue(item: EntityRecord, column: TreeListColumn): PropertyValue {
  const traits = item.traits ?? [];
  const instance = traits.find((t) => t.template_id === column.traitTemplateId);

  if (instance?.values && Object.prototype.hasOwnProperty.call(instance.values, column.property.key)) {
    const raw = instance.values[column.property.key];
    if (raw !== undefined) {
      return raw;
    }
  }

  if (column.property.default !== undefined) {
    return column.property.default;
  }

  if (column.property.type === "json" && !column.property.is_array) {
    return {};
  }

  return defaultPropertyValue(column.property.type, Boolean(column.property.is_array));
}

export function setTreeColumnValue(
  item: EntityRecord,
  column: TreeListColumn,
  value: PropertyValue
): EntityRecord {
  const traits = item.traits ? [...item.traits] : [];
  const matchIndex = traits.findIndex((t) => t.template_id === column.traitTemplateId);

  if (matchIndex === -1) {
    throw new Error(
      `Cannot set tree column value: entity "${item.id}" is missing trait instance for template "${column.traitTemplateId}"`
    );
  }

  const existing = traits[matchIndex]!;
  const nextInstance: TraitInstance = {
    ...existing,
    values: {
      ...(existing.values ?? {}),
      [column.property.key]: value,
    },
  };
  traits[matchIndex] = nextInstance;

  return {
    ...item,
    traits,
  } as EntityRecord;
}
