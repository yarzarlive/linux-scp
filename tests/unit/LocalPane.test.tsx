import { render, screen, waitFor } from "@testing-library/react";
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
});
