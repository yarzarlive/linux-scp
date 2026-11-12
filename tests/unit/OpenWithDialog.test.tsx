import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { OpenWithDialog } from "../../src/components/OpenWithDialog/OpenWithDialog";
import { IpcClient } from "../../src/services/ipc";
import { DesktopAppInfo } from "../../src/types/desktop";

describe("OpenWithDialog Component", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  const sampleApps: DesktopAppInfo[] = [
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
    {
      id: "vim.desktop",
      name: "Vim",
      comment: "Edit text files in terminal",
      icon: "gvim",
      exec_raw: "vim %F",
      executable: "/usr/bin/vim",
      args_template: ["%F"],
      terminal: true,
      mime_types: ["text/plain"],
      categories: ["Utility", "TextEditor"],
      desktop_file_path: "/usr/share/applications/vim.desktop",
    },
    {
      id: "gimp.desktop",
      name: "GNU Image Manipulation Program",
      comment: "Create images and edit photographs",
      icon: "gimp",
      exec_raw: "gimp-2.10 %U",
      executable: "/usr/bin/gimp-2.10",
      args_template: ["%U"],
      terminal: false,
      mime_types: ["image/png"],
      categories: ["Graphics"],
      desktop_file_path: "/usr/share/applications/gimp.desktop",
    },
  ];

  it("discovers applications, filters by search query, and launches with rememberDefault", async () => {
    vi.spyOn(IpcClient, "discoverDesktopApplications").mockResolvedValue({
      extension_key: ".txt",
      extension_display: ".txt",
      remembered_app_id: null,
      remembered_app_installed: false,
      recommended_app_ids: ["org.gnome.TextEditor.desktop", "vim.desktop"],
      applications: sampleApps,
    });

    const openWithAppSpy = vi.spyOn(IpcClient, "openLocalFileWithApp").mockResolvedValue({
      app: sampleApps[1],
      extension_key: ".txt",
      saved_as_default: true,
      pid: 4242,
    });

    const handleClose = vi.fn();
    const handleLaunched = vi.fn();

    render(
      <OpenWithDialog
        isOpen={true}
        filePath="/home/tester/notes.txt"
        fileName="notes.txt"
        onClose={handleClose}
        onLaunched={handleLaunched}
      />
    );

    await waitFor(() => {
      expect(screen.getByText("Text Editor")).toBeInTheDocument();
      expect(screen.getByText("Vim")).toBeInTheDocument();
      expect(screen.getByText("GNU Image Manipulation Program")).toBeInTheDocument();
    });

    // Search filter
    const searchInput = screen.getByLabelText("Filter applications");
    fireEvent.change(searchInput, { target: { value: "Vim" } });

    expect(screen.getByText("Vim")).toBeInTheDocument();
    expect(screen.queryByText("GNU Image Manipulation Program")).not.toBeInTheDocument();

    // Select Vim
    fireEvent.click(screen.getByText("Vim"));

    // Toggle "Remember as LinuxSCP default application"
    const rememberCheckbox = screen.getByRole("checkbox");
    fireEvent.click(rememberCheckbox);
    expect(rememberCheckbox).toBeChecked();

    // Launch
    const openBtn = screen.getByRole("button", { name: /Open File/i });
    fireEvent.click(openBtn);

    await waitFor(() => {
      expect(openWithAppSpy).toHaveBeenCalledWith("/home/tester/notes.txt", "vim.desktop", true);
      expect(handleLaunched).toHaveBeenCalledTimes(1);
      expect(handleClose).toHaveBeenCalledTimes(1);
    });
  });

  it("shows missing remembered default warning and supports clearing stale default", async () => {
    vi.spyOn(IpcClient, "discoverDesktopApplications").mockResolvedValue({
      extension_key: ".txt",
      extension_display: ".txt",
      remembered_app_id: "removed-editor.desktop",
      remembered_app_installed: false,
      recommended_app_ids: ["org.gnome.TextEditor.desktop"],
      applications: sampleApps,
    });

    const deleteAssocSpy = vi
      .spyOn(IpcClient, "deleteFileAppAssociation")
      .mockResolvedValue(undefined);

    render(
      <OpenWithDialog
        isOpen={true}
        filePath="/home/tester/notes.txt"
        fileName="notes.txt"
        missingRememberedAppId="removed-editor.desktop"
        onClose={vi.fn()}
      />
    );

    await waitFor(() => {
      expect(screen.getByText(/removed-editor\.desktop/i)).toBeInTheDocument();
      expect(screen.getByText(/Clear Stale Default/i)).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText(/Clear Stale Default/i));

    await waitFor(() => {
      expect(deleteAssocSpy).toHaveBeenCalledWith(".txt");
    });
  });

  it("displays launch failure errors clearly without closing dialog", async () => {
    vi.spyOn(IpcClient, "discoverDesktopApplications").mockResolvedValue({
      extension_key: "__no_extension__",
      extension_display: "files with no extension",
      remembered_app_id: null,
      remembered_app_installed: false,
      recommended_app_ids: [],
      applications: sampleApps,
    });

    vi.spyOn(IpcClient, "openLocalFileWithApp").mockRejectedValue({
      code: "CommandFailed",
      message: "Failed to launch desktop application 'Text Editor': binary not executable",
    });

    const handleClose = vi.fn();

    render(
      <OpenWithDialog
        isOpen={true}
        filePath="/home/tester/Makefile"
        fileName="Makefile"
        onClose={handleClose}
      />
    );

    await waitFor(() => {
      expect(screen.getByText("Text Editor")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByRole("button", { name: /Open File/i }));

    await waitFor(() => {
      expect(
        screen.getByText(/Failed to launch desktop application 'Text Editor'/i)
      ).toBeInTheDocument();
    });
    expect(handleClose).not.toHaveBeenCalled();
  });
});
