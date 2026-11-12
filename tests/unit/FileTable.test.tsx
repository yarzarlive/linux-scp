import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FileTable } from "../../src/components/FilePanel/FileTable";
import { FileEntry } from "../../src/types/filesystem";

const mockEntries: FileEntry[] = [
  {
    name: "Documents",
    path: "/home/user/Documents",
    file_type: { type: "Directory" },
    metadata: {
      size: 4096,
      modified_at_secs: 1700000000,
      permissions: {
        raw_mode: 0o755,
        user_read: true,
        user_write: true,
        user_execute: true,
        group_read: true,
        group_write: false,
        group_execute: true,
        other_read: true,
        other_write: false,
        other_execute: true,
        set_uid: false,
        set_gid: false,
        sticky: false,
      },
      ownership: { user_name: "user", group_name: "user" },
    },
    is_hidden: false,
  },
  {
    name: "notes.txt",
    path: "/home/user/notes.txt",
    file_type: { type: "Regular" },
    metadata: {
      size: 2048,
      modified_at_secs: 1700001000,
      permissions: {
        raw_mode: 0o644,
        user_read: true,
        user_write: true,
        user_execute: false,
        group_read: true,
        group_write: false,
        group_execute: false,
        other_read: true,
        other_write: false,
        other_execute: false,
        set_uid: false,
        set_gid: false,
        sticky: false,
      },
      ownership: { user_name: "user", group_name: "user" },
    },
    is_hidden: false,
  },
  {
    name: "photo.jpg",
    path: "/home/user/photo.jpg",
    file_type: { type: "Regular" },
    metadata: {
      size: 1048576,
      modified_at_secs: 1700002000,
      permissions: {
        raw_mode: 0o644,
        user_read: true,
        user_write: true,
        user_execute: false,
        group_read: true,
        group_write: false,
        group_execute: false,
        other_read: true,
        other_write: false,
        other_execute: false,
        set_uid: false,
        set_gid: false,
        sticky: false,
      },
      ownership: { user_name: "user", group_name: "user" },
    },
    is_hidden: false,
  },
];

describe("FileTable Component", () => {
  it("renders files and directories with formatted columns", () => {
    render(
      <FileTable
        entries={mockEntries}
        selectedPaths={new Set()}
        onSelectionChange={vi.fn()}
        onOpenEntry={vi.fn()}
        sortBy="name"
        sortDirection="asc"
        onSortChange={vi.fn()}
      />
    );

    expect(screen.getByText("Documents")).toBeInTheDocument();
    expect(screen.getByText("notes.txt")).toBeInTheDocument();
    expect(screen.getByText("photo.jpg")).toBeInTheDocument();

    // Directory size should display as dash
    expect(screen.getByText("—")).toBeInTheDocument();
    // Regular file sizes
    expect(screen.getByText("2.0 KB")).toBeInTheDocument();
    expect(screen.getByText("1.0 MB")).toBeInTheDocument();
  });

  it("handles single selection when row is clicked", () => {
    const handleSelectionChange = vi.fn();
    render(
      <FileTable
        entries={mockEntries}
        selectedPaths={new Set()}
        onSelectionChange={handleSelectionChange}
        onOpenEntry={vi.fn()}
        sortBy="name"
        sortDirection="asc"
        onSortChange={vi.fn()}
      />
    );

    const row = screen.getByText("notes.txt");
    fireEvent.click(row);

    expect(handleSelectionChange).toHaveBeenCalledWith(
      new Set(["/home/user/notes.txt"]),
      "/home/user/notes.txt"
    );
  });

  it("triggers onOpenEntry when a row is double-clicked", () => {
    const handleOpenEntry = vi.fn();
    render(
      <FileTable
        entries={mockEntries}
        selectedPaths={new Set()}
        onSelectionChange={vi.fn()}
        onOpenEntry={handleOpenEntry}
        sortBy="name"
        sortDirection="asc"
        onSortChange={vi.fn()}
      />
    );

    const dirRow = screen.getByText("Documents");
    fireEvent.doubleClick(dirRow);

    expect(handleOpenEntry).toHaveBeenCalledWith(mockEntries[0]);
  });

  it("shows empty state when no entries are present", () => {
    render(
      <FileTable
        entries={[]}
        selectedPaths={new Set()}
        onSelectionChange={vi.fn()}
        onOpenEntry={vi.fn()}
        sortBy="name"
        sortDirection="asc"
        onSortChange={vi.fn()}
      />
    );

    expect(screen.getByText("Directory is empty")).toBeInTheDocument();
  });
});
