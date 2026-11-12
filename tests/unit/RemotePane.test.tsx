import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { RemotePane } from "../../src/components/FilePanel/RemotePane";
import { SessionStateSnapshot } from "../../src/types/session";

describe("RemotePane Component", () => {
  it("renders explicit honest disconnected state with no fabricated files", () => {
    render(<RemotePane />);

    // Shows Disconnected badge and status
    expect(screen.getAllByText("Disconnected").length).toBeGreaterThan(0);
    expect(
      screen.getByText("No Remote Connection Established")
    ).toBeInTheDocument();

    // Shows disabled sftp address bar
    const addressInput = screen.getByLabelText("Path location") as HTMLInputElement;
    expect(addressInput.value).toBe("sftp://[not connected]/");
    expect(addressInput).toBeDisabled();

    // Verifies all 5 column headers exist in disabled mode
    expect(screen.getByTitle("Sort by Name")).toBeDisabled();
    expect(screen.getByTitle("Sort by Size")).toBeDisabled();
    expect(screen.getByTitle("Sort by Changed")).toBeDisabled();
    expect(screen.getByTitle("Sort by Rights")).toBeDisabled();
    expect(screen.getByTitle("Sort by Owner")).toBeDisabled();
  });

  it("calls onConnect when New Connection button is clicked", () => {
    const handleConnect = vi.fn();
    render(<RemotePane onConnect={handleConnect} />);

    const connectButton = screen.getByRole("button", { name: /New Connection.../i });
    fireEvent.click(connectButton);

    expect(handleConnect).toHaveBeenCalledTimes(1);
  });

  it("renders connecting state with cancel action", () => {
    const handleDisconnect = vi.fn();
    const connectingState: SessionStateSnapshot = {
      status: { status: "Connecting" },
      session: null,
      trusted_host_key: null,
    };

    render(
      <RemotePane sessionState={connectingState} onDisconnect={handleDisconnect} />
    );

    expect(screen.getByText("Connecting")).toBeInTheDocument();
    expect(screen.getByText("Establishing SFTP Session...")).toBeInTheDocument();

    const cancelBtn = screen.getByRole("button", { name: /Cancel Connection/i });
    fireEvent.click(cancelBtn);
    expect(handleDisconnect).toHaveBeenCalledTimes(1);
  });

  it("renders connected SFTP session details and verified host key", () => {
    const handleDisconnect = vi.fn();
    const handleReconnect = vi.fn();

    const connectedState: SessionStateSnapshot = {
      status: { status: "Connected" },
      session: {
        session_id: "sess-123",
        profile_id: "prof-1",
        protocol: "SFTP",
        host: "192.168.122.10",
        port: 22,
        username: "ubuntu",
        status: { status: "Connected" },
        elevation: { type: "None" },
        current_remote_path: "/home/ubuntu",
        capabilities: {
          protocol: "SFTP",
          can_list_directory: true,
          can_stat_metadata: true,
          can_upload: true,
          can_download: true,
          can_create_file: true,
          can_create_directory: true,
          can_rename: true,
          can_delete_file: true,
          can_delete_directory: true,
          can_set_permissions: true,
          can_set_owner_group: true,
          can_elevate_sudo: true,
          can_preserve_timestamps: true,
          can_resume_transfer: true,
          can_symlink: true,
        },
        last_active_ms: 1700000000000,
      },
      trusted_host_key: {
        host: "192.168.122.10",
        port: 22,
        algorithm: "ssh-ed25519",
        fingerprint_sha256: "SHA256:VerifiedKey12345",
        public_key_openssh: "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI...",
        trusted_at_ms: 1700000000000,
      },
    };

    render(
      <RemotePane
        sessionState={connectedState}
        onDisconnect={handleDisconnect}
        onReconnect={handleReconnect}
      />
    );

    expect(screen.getByText("Connected")).toBeInTheDocument();
    expect(screen.getByText("SFTP Subsystem Established & Ready")).toBeInTheDocument();
    expect(screen.getByText("ubuntu@192.168.122.10:22")).toBeInTheDocument();
    expect(screen.getByText("/home/ubuntu")).toBeInTheDocument();
    expect(screen.getByText("SHA256:VerifiedKey12345")).toBeInTheDocument();

    const addressInput = screen.getByLabelText("Path location") as HTMLInputElement;
    expect(addressInput.value).toBe("/home/ubuntu");

    fireEvent.click(screen.getByRole("button", { name: /Reconnect/i }));
    expect(handleReconnect).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /Disconnect/i }));
    expect(handleDisconnect).toHaveBeenCalledTimes(1);
  });

  it("renders failed state with actionable error message and retry button", () => {
    const handleReconnect = vi.fn();
    const failedState: SessionStateSnapshot = {
      status: {
        status: "Failed",
        details: { error_message: "Authentication failed for user ubuntu" },
      },
      session: null,
      trusted_host_key: null,
    };

    render(<RemotePane sessionState={failedState} onReconnect={handleReconnect} />);

    expect(screen.getByText("Failed")).toBeInTheDocument();
    expect(screen.getByText("Remote SFTP Connection Failed")).toBeInTheDocument();
    expect(
      screen.getByText("Authentication failed for user ubuntu")
    ).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /Retry Connection/i }));
    expect(handleReconnect).toHaveBeenCalledTimes(1);
  });
});
