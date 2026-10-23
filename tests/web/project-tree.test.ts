import { describe, expect, test } from "bun:test";
import { buildTreeItems, defaultExpandedKeys, flattenTree, folderBreadcrumb, folderContents, isFolderDescendant, itemRoute } from "../../src/web/studio/projectTree.ts";
import type { EntityRecord, FolderRecord } from "../../src/web/studio/contracts.ts";

const folders: FolderRecord[] = [
  { id: "f-characters", name: "Characters", root: "entities", parent_id: null },
  { id: "f-guards", name: "Guards", root: "entities", parent_id: "f-characters" },
  { id: "f-act-one", name: "Act One", root: "storylets", parent_id: null },
  { id: "f-zeta", name: "Zeta", root: "entities", parent_id: null },
  { id: "f-alpha", name: "Alpha", root: "entities", parent_id: null },
];

function entity(id: string, name: string, folderId: string | null, thumbnail: string | null = null): EntityRecord {
  return {
    id,
    object_id: "0x0000000000000001",
    game_id: `Chr_${name}`,
    name,
    type: "character",
    folder_id: folderId,
    thumbnail,
    properties: [],
    traits: [],
    character: { role: "", home_location: "", current_location: "" },
  };
}

const source = buildTreeItems({
  projectName: "Workspace",
  folders,
  qualities: [],
  entities: [entity("guard", "Guard", "f-guards"), entity("boss", "Boss", "f-characters"), entity("loose", "Loose", null)],
  traitTemplates: [],
  assets: [],
  storylets: [],
});

describe("project tree", () => {
  test("nests folders before items and respects expanded branches", () => {
    const expanded = [...defaultExpandedKeys(), "folder:f-characters", "folder:f-guards", "folder:f-act-one"];
    const rows = flattenTree({ ...source, expanded });
    expect(rows.map((row) => row.key)).toEqual([
      "project",
      "root:storylets",
      "folder:f-act-one",
      "root:entities",
      "folder:f-alpha",
      "folder:f-characters",
      "folder:f-guards",
      "item:entities:guard",
      "item:entities:boss",
      "folder:f-zeta",
      "item:entities:loose",
      "root:qualities",
      "root:trait_templates",
      "root:assets",
    ]);
    expect(rows.find((row) => row.key === "folder:f-guards")?.depth).toBe(3);
    expect(rows.find((row) => row.key === "item:entities:guard")?.detail).toBe("Character");
  });

  test("collapsed folders hide their children", () => {
    const rows = flattenTree({ ...source, expanded: defaultExpandedKeys() });
    expect(rows.some((row) => row.key === "folder:f-characters")).toBe(true);
    expect(rows.some((row) => row.key === "folder:f-guards")).toBe(false);
    expect(rows.some((row) => row.key === "item:entities:guard")).toBe(false);
    expect(rows.some((row) => row.key === "item:entities:boss")).toBe(false);
    expect(rows.some((row) => row.key === "item:entities:loose")).toBe(true);
  });

  test("search matches folder names too", () => {
    const rows = flattenTree({ ...source, expanded: [], query: "zeta" });
    expect(rows.map((row) => row.key)).toEqual([
      "project",
      "root:entities",
      "folder:f-zeta",
    ]);
  });

  test("search keeps a nested match and its ancestors", () => {
    const rows = flattenTree({ ...source, expanded: [], query: "guar" });
    expect(rows.map((row) => row.key)).toEqual([
      "project",
      "root:entities",
      "folder:f-characters",
      "folder:f-guards",
      "item:entities:guard",
    ]);
  });

  test("breadcrumb and descendant checks follow the parent chain", () => {
    expect(folderBreadcrumb(folders, "f-guards").map((folder) => folder.name)).toEqual([
      "Characters",
      "Guards",
    ]);
    expect(folderBreadcrumb(folders, null)).toEqual([]);
    expect(isFolderDescendant(folders, "f-guards", "f-characters")).toBe(true);
    expect(isFolderDescendant(folders, "f-characters", "f-guards")).toBe(false);
    expect(isFolderDescendant(folders, "f-characters", "f-characters")).toBe(false);
  });

  test("folder contents split subfolders from items", () => {
    const contents = folderContents(folders, source.items, "entities", "f-characters");
    expect(contents.folders.map((folder) => folder.name)).toEqual(["Guards"]);
    expect(contents.items.map((item) => item.label)).toEqual(["Boss"]);
  });

  test("item routes point at the matching editor", () => {
    expect(itemRoute("entities", "guard")).toBe("/entities/guard");
    expect(itemRoute("storylets", "s 1")).toBe("/storylets/s%201/edit");
    expect(itemRoute("qualities", "Doom")).toBe("/qualities/Doom");
    expect(itemRoute("trait_templates", "t1")).toBe("/trait-templates/t1");
    expect(itemRoute("assets", "a1")).toBe("/assets/a1");
  });
});
