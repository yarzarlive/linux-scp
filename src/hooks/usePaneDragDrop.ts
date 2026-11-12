import { useState, useCallback, useEffect } from "react";
import { FileEntry } from "../types/filesystem";
import {
  INTERNAL_DRAG_MIME,
  InternalDragPayload,
  PaneKind,
  isEntryDraggable,
  isExternalDataTransfer,
  validateInternalDrop,
} from "../utils/dragDrop";

export type PaneDropState =
  | { status: "idle" }
  | {
      status: "valid";
      direction: "upload" | "download";
      targetDirectory: string;
      hoveredEntryPath: string | null;
    }
  | {
      status: "invalid";
      reason: string;
      hoveredEntryPath: string | null;
    };

interface UsePaneDragDropOptions {
  pane: PaneKind;
  currentPath: string;
  isReady: boolean;
  entries: FileEntry[];
  selectedPaths: Set<string>;
  onSelectionChange: (paths: Set<string>, focused?: string) => void;
  activeDrag: InternalDragPayload | null;
  onInternalDragStart?: (payload: InternalDragPayload) => void;
  onInternalDragEnd?: () => void;
  onAcceptDrop?: (sourcePaths: string[], targetDirectory: string) => void;
}

export function usePaneDragDrop({
  pane,
  currentPath,
  isReady,
  entries,
  selectedPaths,
  onSelectionChange,
  activeDrag,
  onInternalDragStart,
  onInternalDragEnd,
  onAcceptDrop,
}: UsePaneDragDropOptions) {
  const [dropState, setDropState] = useState<PaneDropState>({ status: "idle" });

  useEffect(() => {
    if (!activeDrag) {
      setDropState({ status: "idle" });
    }
  }, [activeDrag]);

  const handleEntryDragStart = useCallback(
    (e: React.DragEvent, entry: FileEntry) => {
      if (!isEntryDraggable(entry)) {
        e.preventDefault();
        return;
      }

      let paths: string[];
      if (selectedPaths.has(entry.path)) {
        const selectedDraggable = entries
          .filter((item) => selectedPaths.has(item.path) && isEntryDraggable(item))
          .map((item) => item.path);
        paths = selectedDraggable.length > 0 ? selectedDraggable : [entry.path];
      } else {
        onSelectionChange(new Set([entry.path]), entry.path);
        paths = [entry.path];
      }

      if (e.dataTransfer) {
        e.dataTransfer.effectAllowed = "copy";
        // Only expose an internal opaque marker; never expose filesystem paths or URIs to external desktop apps.
        e.dataTransfer.setData(
          INTERNAL_DRAG_MIME,
          JSON.stringify({ sourcePane: pane, count: paths.length })
        );
      }

      onInternalDragStart?.({ sourcePane: pane, sourcePaths: paths });
    },
    [entries, onInternalDragStart, onSelectionChange, pane, selectedPaths]
  );

  const handleEntryDragEnd = useCallback(() => {
    setDropState({ status: "idle" });
    onInternalDragEnd?.();
  }, [onInternalDragEnd]);

  const evaluateDragOver = useCallback(
    (e: React.DragEvent, hoveredEntry?: FileEntry) => {
      e.preventDefault();
      e.stopPropagation();

      const hasExternalData = isExternalDataTransfer(e.dataTransfer);
      const result = validateInternalDrop({
        dragPayload: activeDrag,
        targetPane: pane,
        targetPaneCurrentPath: currentPath,
        targetPaneReady: isReady,
        hoveredEntry,
        hasExternalData,
      });

      if (e.dataTransfer) {
        e.dataTransfer.dropEffect = result.valid ? "copy" : "none";
      }

      const highlightedFolderPath =
        hoveredEntry && hoveredEntry.file_type.type === "Directory" ? hoveredEntry.path : null;

      if (result.valid) {
        setDropState({
          status: "valid",
          direction: result.direction,
          targetDirectory: result.targetDirectory,
          hoveredEntryPath: highlightedFolderPath,
        });
      } else if (activeDrag && !hasExternalData) {
        setDropState({
          status: "invalid",
          reason: result.reason,
          hoveredEntryPath: highlightedFolderPath,
        });
      } else {
        setDropState({ status: "idle" });
      }
    },
    [activeDrag, currentPath, isReady, pane]
  );

  const handlePaneDragOver = useCallback(
    (e: React.DragEvent) => evaluateDragOver(e, undefined),
    [evaluateDragOver]
  );

  const handleRowDragOver = useCallback(
    (e: React.DragEvent, entry: FileEntry) => evaluateDragOver(e, entry),
    [evaluateDragOver]
  );

  const handlePaneDragLeave = useCallback((e: React.DragEvent) => {
    if (e.relatedTarget && e.currentTarget.contains(e.relatedTarget as Node)) {
      return;
    }
    setDropState({ status: "idle" });
  }, []);

  const handleRowDragLeave = useCallback((e: React.DragEvent) => {
    e.stopPropagation();
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent, hoveredEntry?: FileEntry) => {
      e.preventDefault();
      e.stopPropagation();
      setDropState({ status: "idle" });

      const hasExternalData = isExternalDataTransfer(e.dataTransfer);
      const result = validateInternalDrop({
        dragPayload: activeDrag,
        targetPane: pane,
        targetPaneCurrentPath: currentPath,
        targetPaneReady: isReady,
        hoveredEntry,
        hasExternalData,
      });

      onInternalDragEnd?.();

      if (!result.valid || !activeDrag) {
        return;
      }

      onAcceptDrop?.(activeDrag.sourcePaths, result.targetDirectory);
    },
    [activeDrag, currentPath, isReady, onAcceptDrop, onInternalDragEnd, pane]
  );

  const handlePaneDrop = useCallback(
    (e: React.DragEvent) => handleDrop(e, undefined),
    [handleDrop]
  );

  const handleRowDrop = useCallback(
    (e: React.DragEvent, entry: FileEntry) => handleDrop(e, entry),
    [handleDrop]
  );

  return {
    dropState,
    handleEntryDragStart,
    handleEntryDragEnd,
    handlePaneDragOver,
    handleRowDragOver,
    handlePaneDragLeave,
    handleRowDragLeave,
    handlePaneDrop,
    handleRowDrop,
  };
}
