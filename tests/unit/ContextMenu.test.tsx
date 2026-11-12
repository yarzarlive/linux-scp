import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ContextMenu } from "../../src/components/FilePanel/ContextMenu";

describe("ContextMenu Component", () => {
  it("renders menu options and handles triggers", () => {
    const handleNewFolder = vi.fn();
    const handleNewFile = vi.fn();
    const handleRefresh = vi.fn();
    const handleClose = vi.fn();

    render(
      <ContextMenu
        position={{ x: 100, y: 100 }}
        onClose={handleClose}
        hasSelection={false}
        isSingleSelection={false}
        isDirectorySelection={false}
        onNewFolder={handleNewFolder}
        onNewFile={handleNewFile}
        onRefresh={handleRefresh}
      />
    );

    expect(screen.getByText("New Folder (F7)")).toBeInTheDocument();
    expect(screen.getByText("New File")).toBeInTheDocument();
    expect(screen.getByText("Refresh (F5)")).toBeInTheDocument();

    fireEvent.click(screen.getByText("New Folder (F7)"));
    expect(handleNewFolder).toHaveBeenCalledTimes(1);
    expect(handleClose).toHaveBeenCalled();
  });

  it("renders selection-specific items when item is selected", () => {
    const handleRename = vi.fn();
    const handleDelete = vi.fn();

    render(
      <ContextMenu
        position={{ x: 100, y: 100 }}
        onClose={vi.fn()}
        hasSelection={true}
        isSingleSelection={true}
        isDirectorySelection={false}
        onRename={handleRename}
        onDelete={handleDelete}
      />
    );

    expect(screen.getByText("Rename (F2)")).toBeInTheDocument();
    expect(screen.getByText("Delete (Del)")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Rename (F2)"));
    expect(handleRename).toHaveBeenCalled();
  });

  it("renders Open and Open With... actions when a single file is selected and handlers are provided", () => {
    const handleOpen = vi.fn();
    const handleOpenWith = vi.fn();
    const handleClose = vi.fn();

    render(
      <ContextMenu
        position={{ x: 100, y: 100 }}
        onClose={handleClose}
        hasSelection={true}
        isSingleSelection={true}
        isDirectorySelection={false}
        onOpen={handleOpen}
        onOpenWith={handleOpenWith}
      />
    );

    expect(screen.getByText("Open")).toBeInTheDocument();
    expect(screen.getByText("Open With...")).toBeInTheDocument();

    fireEvent.click(screen.getByText("Open"));
    expect(handleOpen).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("Open With..."));
    expect(handleOpenWith).toHaveBeenCalledTimes(1);
    expect(handleClose).toHaveBeenCalledTimes(2);
  });
});
