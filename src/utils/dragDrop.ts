import { FileEntry } from "../types/filesystem";

export type PaneKind = "local" | "remote";

export const INTERNAL_DRAG_MIME = "application/x-linuxscp-internal";

export interface InternalDragPayload {
  sourcePane: PaneKind;
  sourcePaths: string[];
}

export type DropValidationResult =
  | {
      valid: true;
      direction: "upload" | "download";
      targetDirectory: string;
    }
  | {
      valid: false;
      reason: string;
    };

export function isEntryDraggable(entry: FileEntry): boolean {
  return entry.file_type.type !== "Special";
}

export function isExternalDataTransfer(dataTransfer: DataTransfer | null | undefined): boolean {
  if (!dataTransfer) return false;
  if (dataTransfer.files && dataTransfer.files.length > 0) return true;
  if (dataTransfer.types) {
    const types = Array.from(dataTransfer.types);
    if (types.includes("Files") && !types.includes(INTERNAL_DRAG_MIME)) {
      return true;
    }
    if (types.length > 0 && !types.includes(INTERNAL_DRAG_MIME)) {
      return true;
    }
  }
  return false;
}

export function validateInternalDrop(params: {
  dragPayload: InternalDragPayload | null;
  targetPane: PaneKind;
  targetPaneCurrentPath: string;
  targetPaneReady: boolean;
  hoveredEntry?: FileEntry | null;
  hasExternalData?: boolean;
}): DropValidationResult {
  const {
    dragPayload,
    targetPane,
    targetPaneCurrentPath,
    targetPaneReady,
    hoveredEntry,
    hasExternalData = false,
  } = params;

  if (hasExternalData || !dragPayload || dragPayload.sourcePaths.length === 0) {
    return {
      valid: false,
      reason: "External desktop drag-and-drop is not supported.",
    };
  }

  if (dragPayload.sourcePane === targetPane) {
    return {
      valid: false,
      reason: "Cannot drop items into the same pane.",
    };
  }

  if (!targetPaneReady || !targetPaneCurrentPath) {
    return {
      valid: false,
      reason:
        targetPane === "remote"
          ? "Remote pane must be connected to accept uploads."
          : "Local pane is not ready to accept downloads.",
    };
  }

  let targetDirectory = targetPaneCurrentPath;
  if (hoveredEntry && hoveredEntry.file_type.type === "Directory") {
    targetDirectory = hoveredEntry.path;
  }

  const direction =
    dragPayload.sourcePane === "local" && targetPane === "remote" ? "upload" : "download";

  return {
    valid: true,
    direction,
    targetDirectory,
  };
}
