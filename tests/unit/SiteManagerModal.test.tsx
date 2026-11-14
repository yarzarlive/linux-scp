import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { describe, expect, it, vi, beforeEach } from "vitest";
import { SiteManagerModal } from "../../src/components/SiteManager/SiteManagerModal";
import { IpcClient } from "../../src/services/ipc";
import { ConnectionProfile, ConnectionProfileSummary } from "../../src/types/connection";

const mockSummaryList: ConnectionProfileSummary[] = [
  {
    id: "profile-1",
    name: "Production Web",
    protocol: "SFTP",
    host: "192.168.1.100",
    port: 22,
    username: "deploy",
    has_saved_secret: true,
    updated_at_ms: 1700000000,
  },
  {
    id: "profile-2",
    name: "Backup SCP",
    protocol: "SCP",
    host: "10.0.0.5",
    port: 22,
    username: "backup",
    has_saved_secret: false,
    updated_at_ms: 1700001000,
  },
];

const mockFullProfile1: ConnectionProfile = {
  id: "profile-1",
  name: "Production Web",
  protocol: "SFTP",
  host: "192.168.1.100",
  port: 22,
  username: "deploy",
  auth_method: { type: "Password" },
  initial_local_directory: undefined,
  initial_remote_directory: "/var/www",
  sudo_sftp: { enabled: true, custom_command: "sudo -n /usr/lib/openssh/sftp-server" },
  transfer_settings: {
    preserve_timestamps: true,
    preserve_permissions: true,
    concurrency_limit: 2,
  },
  connection_settings: {
    timeout_seconds: 30,
    keepalive_interval_seconds: 15,
  },
  created_at_ms: 1700000000,
  updated_at_ms: 1700000000,
};

describe("SiteManagerModal Component", () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    vi.spyOn(IpcClient, "listConnectionProfiles").mockResolvedValue(mockSummaryList);
    vi.spyOn(IpcClient, "getConnectionProfile").mockResolvedValue(mockFullProfile1);
    vi.spyOn(IpcClient, "checkKeyringStatus").mockResolvedValue(true);
  });

  it("renders profile list from IPC and loads details of first profile", async () => {
    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Production Web")).toBeInTheDocument();
      expect(screen.getByText("Backup SCP")).toBeInTheDocument();
    });

    // Check editor fields populated from first profile
    const nameInput = screen.getByDisplayValue("Production Web") as HTMLInputElement;
    expect(nameInput).toBeInTheDocument();

    const hostInput = screen.getByDisplayValue("192.168.1.100") as HTMLInputElement;
    expect(hostInput).toBeInTheDocument();

    expect(screen.getByText("Secret Service: Active")).toBeInTheDocument();
  });

  it("validates empty profile name on save and displays error", async () => {
    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByDisplayValue("Production Web")).toBeInTheDocument();
    });

    const nameInput = screen.getByDisplayValue("Production Web");
    fireEvent.change(nameInput, { target: { value: "   " } });

    const saveBtn = screen.getByRole("button", { name: "Save Profile" });
    fireEvent.click(saveBtn);

    expect(screen.getByText("Profile name cannot be empty")).toBeInTheDocument();
  });

  it("saves profile with password secret via IPC", async () => {
    const saveSpy = vi.spyOn(IpcClient, "saveConnectionProfile").mockResolvedValue({
      ...mockFullProfile1,
      name: "Updated Web",
    });

    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByDisplayValue("Production Web")).toBeInTheDocument();
    });

    const nameInput = screen.getByDisplayValue("Production Web");
    fireEvent.change(nameInput, { target: { value: "Updated Web" } });

    const passwordInput = screen.getByPlaceholderText(
      "Enter new password to update stored secret..."
    );
    fireEvent.change(passwordInput, { target: { value: "secret123" } });

    const saveBtn = screen.getByRole("button", { name: "Save Profile" });
    fireEvent.click(saveBtn);

    await waitFor(() => {
      expect(saveSpy).toHaveBeenCalledWith(
        expect.objectContaining({ name: "Updated Web" }),
        "secret123"
      );
    });
  });

  it("handles duplicate profile trigger", async () => {
    const dupSpy = vi.spyOn(IpcClient, "duplicateConnectionProfile").mockResolvedValue({
      ...mockFullProfile1,
      id: "profile-1-copy",
      name: "Production Web (Copy)",
    });

    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByDisplayValue("Production Web")).toBeInTheDocument();
    });

    const dupBtn = screen.getByTitle("Duplicate selected profile");
    fireEvent.click(dupBtn);

    await waitFor(() => {
      expect(dupSpy).toHaveBeenCalledWith("profile-1", "Production Web (Copy)");
    });
  });

  it("handles delete profile with confirmation dialog", async () => {
    const deleteSpy = vi.spyOn(IpcClient, "deleteConnectionProfile").mockResolvedValue();

    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByDisplayValue("Production Web")).toBeInTheDocument();
    });

    // Click delete in sidebar
    const deleteBtn = screen.getByTitle("Delete selected profile");
    fireEvent.click(deleteBtn);

    // Verify confirmation modal opens
    expect(screen.getByText("Delete Connection Profile")).toBeInTheDocument();

    // Confirm deletion
    const confirmBtn = screen.getByRole("button", { name: "Delete Profile" });
    fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(deleteSpy).toHaveBeenCalledWith("profile-1");
    });
  });

  it("supports launching external SSH terminal for the selected profile", async () => {
    vi.spyOn(IpcClient, "saveConnectionProfile").mockResolvedValue(mockFullProfile1);
    const onOpenTerminal = vi.fn();

    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} onOpenTerminal={onOpenTerminal} />);

    await waitFor(() => {
      expect(screen.getByDisplayValue("Production Web")).toBeInTheDocument();
    });

    const openTermBtn = screen.getByRole("button", { name: /Open Terminal/i });
    expect(openTermBtn).not.toBeDisabled();
    fireEvent.click(openTermBtn);

    await waitFor(() => {
      expect(onOpenTerminal).toHaveBeenCalledWith(mockFullProfile1);
    });
  });

  it("displays narrow sudoers instruction and binary path field when sudo-SFTP is enabled", async () => {
    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByDisplayValue("Production Web")).toBeInTheDocument();
    });

    // Check sudo-sftp checkbox is checked (from mockFullProfile1)
    const sudoCheck = screen.getByLabelText(/Enable Sudo-SFTP External Server Elevation/i);
    expect(sudoCheck).toBeChecked();

    // Check guidance text and sudoers rule
    expect(screen.getByText(/Server-side sudoers configuration required:/i)).toBeInTheDocument();
    expect(
      screen.getByText(/deploy ALL=\(root\) NOPASSWD: \/usr\/lib\/openssh\/sftp-server/i)
    ).toBeInTheDocument();
    expect(screen.getByText(/Never configure unrestricted/i)).toBeInTheDocument();
  });

  it("connects to profile and closes modal on double click", async () => {
    const onSelectAndConnect = vi.fn();
    const onClose = vi.fn();

    render(
      <SiteManagerModal isOpen={true} onClose={onClose} onSelectAndConnect={onSelectAndConnect} />
    );

    await waitFor(() => {
      expect(screen.getByText("Production Web")).toBeInTheDocument();
    });

    const profileItem = screen.getByTestId("profile-item-profile-1");
    fireEvent.doubleClick(profileItem);

    await waitFor(() => {
      expect(onSelectAndConnect).toHaveBeenCalledWith(mockFullProfile1);
      expect(onClose).toHaveBeenCalled();
    });
  });

  it("opens context menu with Connect, Edit, and Delete on right click", async () => {
    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Production Web")).toBeInTheDocument();
    });

    const profileItem = screen.getByTestId("profile-item-profile-1");
    fireEvent.contextMenu(profileItem, { clientX: 100, clientY: 150 });

    const menu = screen.getByRole("menu", { name: "Connection Context Menu" });
    expect(menu).toBeInTheDocument();

    expect(screen.getByRole("menuitem", { name: /Connect/i })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /Edit/i })).toBeInTheDocument();
    expect(screen.getByRole("menuitem", { name: /Delete/i })).toBeInTheDocument();
  });

  it("triggers connection from context menu Connect option", async () => {
    const onSelectAndConnect = vi.fn();
    const onClose = vi.fn();

    render(
      <SiteManagerModal isOpen={true} onClose={onClose} onSelectAndConnect={onSelectAndConnect} />
    );

    await waitFor(() => {
      expect(screen.getByText("Production Web")).toBeInTheDocument();
    });

    const profileItem = screen.getByTestId("profile-item-profile-1");
    fireEvent.contextMenu(profileItem, { clientX: 100, clientY: 150 });

    const connectOption = screen.getByRole("menuitem", { name: /Connect/i });
    fireEvent.click(connectOption);

    await waitFor(() => {
      expect(onSelectAndConnect).toHaveBeenCalledWith(mockFullProfile1);
      expect(onClose).toHaveBeenCalled();
    });
  });

  it("selects profile and focuses name input from context menu Edit option", async () => {
    const mockFullProfile2: ConnectionProfile = {
      id: "profile-2",
      name: "Backup SCP",
      protocol: "SCP",
      host: "10.0.0.5",
      port: 22,
      username: "backup",
      auth_method: { type: "Password" },
      sudo_sftp: { enabled: false },
      transfer_settings: {
        preserve_timestamps: true,
        preserve_permissions: true,
        concurrency_limit: 2,
      },
      connection_settings: {
        timeout_seconds: 30,
        keepalive_interval_seconds: 15,
      },
      created_at_ms: 1700001000,
      updated_at_ms: 1700001000,
    };

    vi.spyOn(IpcClient, "getConnectionProfile").mockImplementation(async (id: string) => {
      if (id === "profile-2") return mockFullProfile2;
      return mockFullProfile1;
    });

    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Backup SCP")).toBeInTheDocument();
    });

    const profile2Item = screen.getByTestId("profile-item-profile-2");
    fireEvent.contextMenu(profile2Item, { clientX: 120, clientY: 180 });

    const editOption = screen.getByRole("menuitem", { name: /Edit/i });
    fireEvent.click(editOption);

    await waitFor(() => {
      expect(screen.getByDisplayValue("Backup SCP")).toBeInTheDocument();
    });
  });

  it("opens delete confirmation and deletes targeted profile from context menu Delete option", async () => {
    const deleteSpy = vi.spyOn(IpcClient, "deleteConnectionProfile").mockResolvedValue();

    render(<SiteManagerModal isOpen={true} onClose={vi.fn()} />);

    await waitFor(() => {
      expect(screen.getByText("Production Web")).toBeInTheDocument();
    });

    const profileItem = screen.getByTestId("profile-item-profile-1");
    fireEvent.contextMenu(profileItem, { clientX: 100, clientY: 150 });

    const deleteOption = screen.getByRole("menuitem", { name: /Delete/i });
    fireEvent.click(deleteOption);

    // Verify confirmation dialog opens for profile-1
    expect(screen.getByText("Delete Connection Profile")).toBeInTheDocument();
    expect(
      screen.getByText(/Are you sure you want to permanently delete profile "Production Web"/i)
    ).toBeInTheDocument();

    const confirmBtn = screen.getByRole("button", { name: "Delete Profile" });
    fireEvent.click(confirmBtn);

    await waitFor(() => {
      expect(deleteSpy).toHaveBeenCalledWith("profile-1");
    });
  });
});
