import { describe, expect, test } from "bun:test";
import {
  characterRecordFromEntity,
  defaultCharacterSection,
  defaultLocationSection,
  locationRecordFromEntity,
  newEntityDraft,
  withEntityType,
} from "../../src/web/studio/entityModel.ts";
import type { EntityRecord } from "../../src/web/studio/contracts.ts";

function character(): EntityRecord {
  return {
    id: "guard",
    object_id: "0x00000000000000AA",
    game_id: "Chr_Guard",
    name: "Guard",
    type: "character",
    description: "Watchful.",
    folder_id: null,
    properties: [],
    traits: [],
    character: defaultCharacterSection(),
  };
}

describe("entity model", () => {
  test("switching to a location drops character settings", () => {
    const location = withEntityType(character(), "location");
    expect(location.type).toBe("location");
    expect(location.character).toBeUndefined();
    expect(location.location).toEqual(defaultLocationSection());
  });

  test("switching kind never discards a still-relevant section twice", () => {
    const location = withEntityType(character(), "location");
    const back = withEntityType(location, "character");
    expect(back.location).toBeUndefined();
    expect(back.character).toEqual(defaultCharacterSection());
  });

  test("item and fact drop both typed sections", () => {
    for (const type of ["item", "fact", "other"] as const) {
      const entity = withEntityType(character(), type);
      expect(entity.character).toBeUndefined();
      expect(entity.location).toBeUndefined();
    }
  });

  test("runtime records are derived from the typed section", () => {
    const record = characterRecordFromEntity(character());
    expect(record).toMatchObject({ id: "guard", name: "Guard", role: "" });
    expect(locationRecordFromEntity(character())).toBeNull();

    const locationEntity = withEntityType(
      { ...character(), description: "Platform nine" },
      "location",
    );
    const locationRecord = locationRecordFromEntity(locationEntity);
    expect(locationRecord).toMatchObject({ id: "guard", name: "Guard", description: "Platform nine" });
    expect(characterRecordFromEntity(locationEntity)).toBeNull();
  });

  test("new drafts start unsaved but typed", () => {
    const draft = newEntityDraft("f-1");
    expect(draft).toMatchObject({
      id: "",
      object_id: "",
      game_id: "",
      name: "New Entity",
      type: "character",
      folder_id: "f-1",
    });
    expect(draft.character).toEqual(defaultCharacterSection());
  });
});
