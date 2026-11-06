import { expect, test } from "bun:test";
import { participantOptions, togglePlayedBy } from "../../src/web/studio/storyletModel.ts";
import type { EntityTemplateRecord } from "../../src/web/studio/contracts.ts";

const entity = (id: string, traits: string[] = []) =>
  ({ id, name: id.toUpperCase(), root_tag: "character", traits: traits.map((template_id) => ({ id: "t", template_id, values: {} })) }) as unknown as EntityTemplateRecord;

test("participantOptions lists only entities with the participant trait, by name", () => {
  const options = participantOptions([entity("rosa", ["system.participant"]), entity("crate"), entity("anna", ["system.participant"])]);
  expect(options).toEqual([{ id: "anna", name: "ANNA" }, { id: "rosa", name: "ROSA" }]);
});

test("togglePlayedBy adds and removes without duplicates and keeps order stable", () => {
  expect(togglePlayedBy([], "rosa", true)).toEqual(["rosa"]);
  expect(togglePlayedBy(["rosa"], "rosa", true)).toEqual(["rosa"]);
  expect(togglePlayedBy(["rosa", "anna"], "rosa", false)).toEqual(["anna"]);
  expect(togglePlayedBy(["anna"], "zed", false)).toEqual(["anna"]);
});
