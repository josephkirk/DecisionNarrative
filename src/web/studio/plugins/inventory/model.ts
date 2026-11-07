import type { TraitInstance, TraitTemplateRecord } from "../../contracts.ts";
import type { AnyEntity } from "../runtime.ts";

export const INVENTORY_OWNER_KEY = "inventory_owner";
export const ITEM_KEY = "item";
export const DEFAULT_COLUMNS = 4;
export const DEFAULT_ROWS = 4;
export const MAX_GRID_DIMENSION = 20;

export interface Inventory {
  columns: number;
  rows: number;
  items: string[];
}

export function inventoryOwnerTemplate(): TraitTemplateRecord {
  return {
    id: INVENTORY_OWNER_KEY, key: INVENTORY_OWNER_KEY, name: "InventoryOwner",
    description: "Owns an inventory grid containing Item entities.", allow_multiple: false,
    properties: [
      { id: "columns", key: "columns", name: "Columns", type: "int", default: DEFAULT_COLUMNS, min: 1, max: MAX_GRID_DIMENSION },
      { id: "rows", key: "rows", name: "Rows", type: "int", default: DEFAULT_ROWS, min: 1, max: MAX_GRID_DIMENSION },
      { id: "items", key: "items", name: "Items", type: "entity", is_array: true, default: [] },
    ],
  };
}

export function itemTemplate(): TraitTemplateRecord {
  return { id: ITEM_KEY, key: ITEM_KEY, name: "Item", description: "Allows this entity to be placed in inventories.", allow_multiple: false, properties: [] };
}

export function traitInstance(entity: AnyEntity, templates: TraitTemplateRecord[], key: string): TraitInstance | undefined {
  const template = templates.find((candidate) => candidate.key === key);
  return template && entity.traits.find((instance) => instance.template_id === template.id);
}

export function inventoryOf(instance: TraitInstance): Inventory {
  const { columns, rows, items } = instance.values;
  return {
    columns: typeof columns === "number" ? columns : DEFAULT_COLUMNS,
    rows: typeof rows === "number" ? rows : DEFAULT_ROWS,
    items: Array.isArray(items) ? items.filter((item): item is string => typeof item === "string") : [],
  };
}

export function inventoryErrors(inventory: Inventory): string[] {
  const validDimension = (value: number) => Number.isInteger(value) && value >= 1 && value <= MAX_GRID_DIMENSION;
  const errors: string[] = [];
  if (!validDimension(inventory.columns) || !validDimension(inventory.rows)) errors.push(`Rows and columns must be between 1 and ${MAX_GRID_DIMENSION}.`);
  if (inventory.items.some((item) => !item) || new Set(inventory.items).size !== inventory.items.length) errors.push("Inventory items must be unique, nonempty entity IDs.");
  if (validDimension(inventory.columns) && validDimension(inventory.rows) && inventory.items.length > inventory.columns * inventory.rows) errors.push("The grid is too small for its items.");
  return errors;
}

export function setInventory(entity: AnyEntity, instance: TraitInstance, inventory: Inventory): Partial<AnyEntity> {
  return { traits: entity.traits.map((trait) => trait.id === instance.id
    ? { ...trait, values: { ...trait.values, columns: inventory.columns, rows: inventory.rows, items: inventory.items } }
    : trait) };
}
