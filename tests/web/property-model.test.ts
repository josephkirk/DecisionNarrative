import { describe, expect, test } from "bun:test";
import {
  defaultPropertyValue,
  propertyListProblem,
  isValidPropertyKey,
  isValidPropertyValue,
  newEntityProperty,
  retypeValue,
  toggleArray,
  uniquePropertyKey,
} from "../../src/web/studio/propertyModel.ts";
import type { PropertyDefinition } from "../../src/web/studio/contracts.ts";

describe("property model", () => {
  test("defaults follow the property type and arity", () => {
    expect(defaultPropertyValue("bool", false)).toBe(false);
    expect(defaultPropertyValue("string", false)).toBe("");
    expect(defaultPropertyValue("int", false)).toBe(0);
    expect(defaultPropertyValue("asset", false)).toBeNull();
    expect(defaultPropertyValue("entity", false)).toBeNull();
    expect(defaultPropertyValue("storylet", false)).toBeNull();
    expect(defaultPropertyValue("tag", false)).toBe("");
  });

  test("value shapes are validated per type", () => {
    expect(isValidPropertyValue("int", false, 3)).toBe(true);
    expect(isValidPropertyValue("int", false, 3.5)).toBe(false);
    expect(isValidPropertyValue("float", false, 3.5)).toBe(true);
    expect(isValidPropertyValue("asset", false, { asset_id: "a" })).toBe(true);
    expect(isValidPropertyValue("asset", false, { asset_id: "a", version: "latest" })).toBe(true);
    expect(isValidPropertyValue("asset", false, {})).toBe(false);
    expect(isValidPropertyValue("entity", true, ["guard", "boss"])).toBe(true);
    expect(isValidPropertyValue("entity", true, ["guard", null])).toBe(false);
    expect(isValidPropertyValue("storylet", false, "opening")).toBe(true);
    expect(isValidPropertyValue("storylet", false, null)).toBe(true);
    expect(isValidPropertyValue("tag", false, "character.hero")).toBe(true);
    expect(isValidPropertyValue("tag", false, null)).toBe(false);
    expect(isValidPropertyValue("string", false, ["a"])).toBe(false);
  });

  test("retyping keeps compatible values and resets the rest", () => {
    expect(retypeValue("float", false, 2)).toBe(2);
    expect(retypeValue("string", false, 2)).toBe("");
    expect(retypeValue("entity", true, ["guard"])).toEqual(["guard"]);
  });

  test("toggling arity converts between scalar and array", () => {
    expect(toggleArray(false, true, "bool")).toEqual([]);
    expect(toggleArray(5, true, "int")).toEqual([]);
    expect(toggleArray([7, 9], false, "int")).toBe(7);
    expect(toggleArray([], false, "int")).toBe(0);
    expect(toggleArray(5, false, "int")).toBe(5);
  });

  test("keys are unique identifiers", () => {
    expect(isValidPropertyKey("Bravery")).toBe(true);
    expect(isValidPropertyKey("2fast")).toBe(false);
    expect(isValidPropertyKey("has space")).toBe(false);
    expect(uniquePropertyKey([])).toBe("NewVar");
    expect(uniquePropertyKey(["NewVar"])).toBe("NewVar_1");
    expect(uniquePropertyKey(["newvar", "newvar_1"])).toBe("NewVar_2");
  });

  test("property lists report the first blocking problem", () => {
    expect(propertyListProblem([])).toBeNull();
    expect(propertyListProblem([{ key: "Age", name: "Age" }])).toBeNull();
    expect(propertyListProblem([{ key: "Age", name: "Age" }, { key: "Age", name: "Age again" }]))
      .toBe("Duplicate property key 'Age'");
    expect(propertyListProblem([{ key: "2bad", name: "Bad" }])).toBe("Property keys must be identifiers");
    expect(propertyListProblem([{ key: "Age", name: "  " }])).toBe("Property 'Age' needs a name");
  });

  test("new properties get a unique key and a boolean default", () => {
    const property = newEntityProperty([{ id: "p", key: "NewVar", name: "NewVar", type: "string" }]);
    expect(property.key).toBe("NewVar_1");
    expect(property.type).toBe("bool");
    expect(property.value).toBe(false);
    expect(property.id.length).toBeGreaterThan(0);
  });

});
