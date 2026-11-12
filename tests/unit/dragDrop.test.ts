import { describe, expect, it } from "vitest";
import { FileEntry } from "../../src/types/filesystem";
import {
  INTERNAL_DRAG_MIME,
  isEntryDraggable,
  isExternalDataTransfer,
  validateInternalDrop,
} from "../../src/utils/dragDrop";

function makeEntry(name: string, type: "Regular" | "Directory" | "Special"): FileEntry {
  return {
    name,
    path: `/dir/${name}`,
    file_type: { type },
    metadata: {
      size: type === "Directory" ? undefined : 128,
      modified_at_secs: 1700000000,
    },
    is_hidden: false,
  };
}

describe("dragDrop utilities", () => {
  it("allows dragging regular files and directories, but rejects Special items", () => {
    expect(isEntryDraggable(makeEntry("notes.txt", "Regular"))).toBe(true);
    expect(isEntryDraggable(makeEntry("folder", "Directory"))).toBe(true);
    expect(isEntryDraggable(makeEntry("socket.sock", "Special"))).toBe(false);
  });

  it("validates local -> remote as upload to pane directory or hovered subfolder", () => {
    const folder = makeEntry("sub", "Directory");
    const paneDrop = validateInternalDrop({
      dragPayload: { sourcePane: "local", sourcePaths: ["/local/a.txt"] },
      targetPane: "remote",
      targetPaneCurrentPath: "/remote/work",
      targetPaneReady: true,
    });
    expect(paneDrop).toEqual({
      valid: true,
      direction: "upload",
      targetDirectory: "/remote/work",
    });

    const folderDrop = validateInternalDrop({
      dragPayload: { sourcePane: "local", sourcePaths: ["/local/a.txt"] },
      targetPane: "remote",
      targetPaneCurrentPath: "/remote/work",
      targetPaneReady: true,
      hoveredEntry: folder,
    });
    expect(folderDrop).toEqual({
      valid: true,
      direction: "upload",
      targetDirectory: "/dir/sub",
    });
  });

  it("validates remote -> local as download", () => {
    const result = validateInternalDrop({
      dragPayload: { sourcePane: "remote", sourcePaths: ["/remote/b.txt", "/remote/c.txt"] },
      targetPane: "local",
      targetPaneCurrentPath: "/home/user/Downloads",
      targetPaneReady: true,
    });
    expect(result).toEqual({
      valid: true,
      direction: "download",
      targetDirectory: "/home/user/Downloads",
    });
  });

  it("treats dropping onto a non-directory file row as dropping into the currently opened directory", () => {
    expect(
      validateInternalDrop({
        dragPayload: { sourcePane: "local", sourcePaths: ["/local/a.txt"] },
        targetPane: "remote",
        targetPaneCurrentPath: "/remote",
        targetPaneReady: true,
        hoveredEntry: makeEntry("file.txt", "Regular"),
      })
    ).toEqual({
      valid: true,
      direction: "upload",
      targetDirectory: "/remote",
    });
  });

  it("rejects same-pane drops, unready panes, and external drops", () => {
    expect(
      validateInternalDrop({
        dragPayload: { sourcePane: "local", sourcePaths: ["/local/a.txt"] },
        targetPane: "local",
        targetPaneCurrentPath: "/local",
        targetPaneReady: true,
      }).valid
    ).toBe(false);

    expect(
      validateInternalDrop({
        dragPayload: { sourcePane: "local", sourcePaths: ["/local/a.txt"] },
        targetPane: "remote",
        targetPaneCurrentPath: "",
        targetPaneReady: false,
      }).valid
    ).toBe(false);

    expect(
      validateInternalDrop({
        dragPayload: null,
        targetPane: "local",
        targetPaneCurrentPath: "/local",
        targetPaneReady: true,
        hasExternalData: true,
      }).valid
    ).toBe(false);
  });

  it("detects external DataTransfer payloads while allowing internal MIME", () => {
    const externalDt = {
      files: [{}],
      types: ["Files"],
    } as unknown as DataTransfer;
    expect(isExternalDataTransfer(externalDt)).toBe(true);

    const internalDt = {
      files: [],
      types: [INTERNAL_DRAG_MIME],
    } as unknown as DataTransfer;
    expect(isExternalDataTransfer(internalDt)).toBe(false);
  });
});
