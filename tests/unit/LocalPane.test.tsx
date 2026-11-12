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
});
