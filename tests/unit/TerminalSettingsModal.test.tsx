import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { TerminalSettingsModal } from "../../src/components/TerminalSettings/TerminalSettingsModal";
import { IpcClient } from "../../src/services/ipc";
import { TerminalEnvironmentInfo } from "../../src/types/terminal";

const mockEnv: TerminalEnvironmentInfo = {
  preference: {
    mode: "auto",
    preset_binary: null,
    custom_executable: null,
    custom_args_prefix: ["-e"],
  },
  supported_terminals: [
    {
      binary: "x-terminal-emulator",
      display_name: "System Default (x-terminal-emulator)",
      exec_prefix_args: ["-e"],
      installed: true,
      resolved_path: "/usr/bin/x-terminal-emulator",
    },
    {
      binary: "gnome-terminal",
      display_name: "GNOME Terminal (gnome-terminal)",
      exec_prefix_args: ["--"],
      installed: true,
      resolved_path: "/usr/bin/gnome-terminal",
    },
    {
      binary: "konsole",
      display_name: "KDE Konsole (konsole)",
      exec_prefix_args: ["-e"],
      installed: false,
      resolved_path: null,
    },
  ],
  detected_default: {
    binary: "x-terminal-emulator",
    display_name: "System Default (x-terminal-emulator)",
    exec_prefix_args: ["-e"],
    installed: true,
    resolved_path: "/usr/bin/x-terminal-emulator",
  },
  ssh_binary_path: "/usr/bin/ssh",
};

describe("TerminalSettingsModal Component", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(IpcClient, "getTerminalEnvironment").mockResolvedValue(mockEnv);
  });

  it("loads and displays detected OpenSSH path and terminal emulators", async () => {
    render(<TerminalSettingsModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("/usr/bin/ssh")).toBeInTheDocument();
      expect(screen.getByText("Detected: x-terminal-emulator")).toBeInTheDocument();
    });
  });

  it("allows selecting a supported preset terminal and saving preference", async () => {
    const saveSpy = vi
      .spyOn(IpcClient, "saveTerminalPreference")
      .mockImplementation(async (p) => p);
    const onClose = vi.fn();

    render(<TerminalSettingsModal isOpen={true} onClose={onClose} />);

    await waitFor(() => {
      expect(screen.getByText("/usr/bin/ssh")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("Choose from Supported Terminal Emulators"));
    const select = screen.getByLabelText("Supported Terminal Emulator");
    fireEvent.change(select, { target: { value: "gnome-terminal" } });

    fireEvent.click(screen.getByRole("button", { name: "Save Terminal Settings" }));

    await waitFor(() => {
      expect(saveSpy).toHaveBeenCalledWith({
        mode: "preset",
        preset_binary: "gnome-terminal",
        custom_executable: null,
        custom_args_prefix: ["-e"],
      });
      expect(onClose).toHaveBeenCalledTimes(1);
    });
  });

  it("validates custom terminal executable and saves distinct argument prefix", async () => {
    const saveSpy = vi
      .spyOn(IpcClient, "saveTerminalPreference")
      .mockImplementation(async (p) => p);

    render(<TerminalSettingsModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("/usr/bin/ssh")).toBeInTheDocument();
    });

    fireEvent.click(screen.getByText("Custom Terminal Executable"));

    // Empty executable should show validation error
    fireEvent.click(screen.getByRole("button", { name: "Save Terminal Settings" }));
    expect(screen.getByText("Custom terminal executable cannot be empty.")).toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Terminal Executable or Path"), {
      target: { value: "/usr/bin/alacritty" },
    });
    fireEvent.change(screen.getByLabelText("Exec Flag Prefix"), {
      target: { value: "--hold -e" },
    });

    fireEvent.click(screen.getByRole("button", { name: "Save Terminal Settings" }));

    await waitFor(() => {
      expect(saveSpy).toHaveBeenCalledWith({
        mode: "custom",
        preset_binary: null,
        custom_executable: "/usr/bin/alacritty",
        custom_args_prefix: ["--hold", "-e"],
      });
    });
  });
});
