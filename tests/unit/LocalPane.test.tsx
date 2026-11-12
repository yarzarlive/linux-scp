import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { LocalPane } from "../../src/components/FilePanel/LocalPane";
import { IpcClient } from "../../src/services/ipc";

describe("LocalPane Component", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it("loads home directory and renders authentic local file entries", async () => {
    vi.spyOn(IpcClient, "getHomeDirectory").mockResolvedValue("/home/tester");
    vi.spyOn(IpcClient, "listLocalDirectory").mockResolvedValue({
      current_path: "/home/tester",
      parent_path: "/home",
      entries: [
        {
          name: "Projects",
          path: "/home/tester/Projects",
          file_type: { type: "Directory" },
          metadata: { size: 4096 },
          is_hidden: false,
        },
        {
          name: "config.json",
          path: "/home/tester/config.json",
          file_type: { type: "Regular" },
          metadata: { size: 512 },
          is_hidden: false,
        },
      ],
    });

    render(<LocalPane />);

    await waitFor(() => {
      expect(screen.getByText("Projects")).toBeInTheDocument();
      expect(screen.getByText("config.json")).toBeInTheDocument();
    });

    expect(screen.getByText("2 items")).toBeInTheDocument();
    expect(screen.getByText("Hidden files hidden")).toBeInTheDocument();
  });

  it("triggers create directory flow", async () => {
    vi.spyOn(IpcClient, "getHomeDirectory").mockResolvedValue("/home/tester");
    vi.spyOn(IpcClient, "listLocalDirectory").mockResolvedValue({
      current_path: "/home/tester",
      parent_path: "/home",
      entries: [],
    });
    const createDirSpy = vi
      .spyOn(IpcClient, "createLocalDirectory")
      .mockResolvedValue("/home/tester/new_folder");

    render(<LocalPane />);

    await waitFor(() => {
      expect(screen.getByText("Directory is empty")).toBeInTheDocument();
    });

    // Click New Folder button
    fireEvent.click(screen.getByTitle("New Folder (F7)"));

    // Verify modal is open
    expect(screen.getByRole("heading", { name: "Create New Folder" })).toBeInTheDocument();
    const input = screen.getByRole("textbox", { name: "Create New Folder" });
    fireEvent.change(input, { target: { value: "new_folder" } });
    fireEvent.submit(input);

    await waitFor(() => {
      expect(createDirSpy).toHaveBeenCalledWith("/home/tester", "new_folder");
    });
  });

  it("triggers delete flow with confirmation modal", async () => {
    vi.spyOn(IpcClient, "getHomeDirectory").mockResolvedValue("/home/tester");
    vi.spyOn(IpcClient, "listLocalDirectory").mockResolvedValue({
      current_path: "/home/tester",
      parent_path: "/home",
      entries: [
        {
          name: "unwanted.txt",
          path: "/home/tester/unwanted.txt",
          file_type: { type: "Regular" },
          metadata: { size: 100 },
          is_hidden: false,
        },
      ],
    });
    const deleteSpy = vi
      .spyOn(IpcClient, "deleteLocalEntries")
      .mockResolvedValue(["/home/tester/unwanted.txt"]);

    render(<LocalPane />);

    await waitFor(() => {
      expect(screen.getByText("unwanted.txt")).toBeInTheDocument();
    });

    // Select unwanted.txt
    fireEvent.click(screen.getByText("unwanted.txt"));

    // Click Delete button
    const deleteBtn = screen.getByTitle("Delete Selected Items (Del)");
    expect(deleteBtn).not.toBeDisabled();
    fireEvent.click(deleteBtn);

    // Verify confirmation dialog is visible
    expect(screen.getByText("Confirm Permanent Deletion")).toBeInTheDocument();

    // Confirm deletion
    const confirmBtn = screen.getByRole("button", { name: "Delete Permanently" });
    fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(deleteSpy).toHaveBeenCalledWith(["/home/tester/unwanted.txt"]);
    });
  });

  it("opens local file with default application or shows OpenWithDialog when NeedsAppPicker", async () => {
    vi.spyOn(IpcClient, "getHomeDirectory").mockResolvedValue("/home/tester");
    vi.spyOn(IpcClient, "listLocalDirectory").mockResolvedValue({
      current_path: "/home/tester",
      parent_path: "/home",
      entries: [
        {
          name: "notes.txt",
          path: "/home/tester/notes.txt",
          file_type: { type: "Regular" },
          metadata: { size: 120 },
          is_hidden: false,
        },
      ],
    });
    const openDefaultSpy = vi.spyOn(IpcClient, "openLocalFileDefault").mockResolvedValue({
      outcome: "NeedsAppPicker",
      extension_key: ".txt",
      extension_display: ".txt",
      missing_remembered_app_id: null,
    });
    const discoverSpy = vi.spyOn(IpcClient, "discoverDesktopApplications").mockResolvedValue({
      extension_key: ".txt",
      extension_display: ".txt",
      remembered_app_id: null,
      remembered_app_installed: false,
      recommended_app_ids: ["org.gnome.TextEditor.desktop"],
      applications: [
        {
          id: "org.gnome.TextEditor.desktop",
          name: "Text Editor",
          comment: "Edit text files",
          icon: "org.gnome.TextEditor",
          exec_raw: "gnome-text-editor %U",
          executable: "/usr/bin/gnome-text-editor",
          args_template: ["%U"],
          terminal: false,
          mime_types: ["text/plain"],
          categories: ["Utility", "TextEditor"],
          desktop_file_path: "/usr/share/applications/org.gnome.TextEditor.desktop",
        },
      ],
    });

    render(<LocalPane />);

    await waitFor(() => {
      expect(screen.getByText("notes.txt")).toBeInTheDocument();
    });

    // Double-click notes.txt to open it
    fireEvent.doubleClick(screen.getByText("notes.txt"));

    await waitFor(() => {
      expect(openDefaultSpy).toHaveBeenCalledWith("/home/tester/notes.txt");
      expect(discoverSpy).toHaveBeenCalledWith("/home/tester/notes.txt");
      expect(
        screen.getByRole("dialog", { name: /Open With — Choose Application/i })
      ).toBeInTheDocument();
      expect(screen.getByText("Text Editor")).toBeInTheDocument();
    });
  });

  it("opens OpenWithDialog via context menu Open With... even when a default exists", async () => {
    vi.spyOn(IpcClient, "getHomeDirectory").mockResolvedValue("/home/tester");
    vi.spyOn(IpcClient, "listLocalDirectory").mockResolvedValue({
      current_path: "/home/tester",
      parent_path: "/home",
      entries: [
        {
          name: "README.md",
          path: "/home/tester/README.md",
          file_type: { type: "Regular" },
          metadata: { size: 256 },
          is_hidden: false,
        },
      ],
    });
    const discoverSpy = vi.spyOn(IpcClient, "discoverDesktopApplications").mockResolvedValue({
      extension_key: ".md",
      extension_display: ".md",
      remembered_app_id: "code.desktop",
      remembered_app_installed: true,
      recommended_app_ids: ["code.desktop"],
      applications: [
        {
          id: "code.desktop",
          name: "Visual Studio Code",
          comment: "Code Editing",
          icon: "vscode",
          exec_raw: "/usr/share/code/code %F",
          executable: "/usr/share/code/code",
          args_template: ["%F"],
          terminal: false,
          mime_types: ["text/plain", "text/markdown"],
          categories: ["Development"],
          desktop_file_path: "/usr/share/applications/code.desktop",
        },
      ],
    });

    render(<LocalPane />);

    await waitFor(() => {
      expect(screen.getByText("README.md")).toBeInTheDocument();
    });

    // Right-click README.md row to select and open context menu
    fireEvent.contextMenu(screen.getByText("README.md"));

    await waitFor(() => {
      expect(screen.getByText("Open")).toBeInTheDocument();
      expect(screen.getByText("Open With...")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("Open With..."));

    await waitFor(() => {
      expect(discoverSpy).toHaveBeenCalledWith("/home/tester/README.md");
      expect(
        screen.getByRole("dialog", { name: /Open With — Choose Application/i })
      ).toBeInTheDocument();
      expect(screen.getAllByText("Visual Studio Code").length).toBeGreaterThan(0);
    });
  });
});
