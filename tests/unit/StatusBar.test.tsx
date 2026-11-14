import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { StatusBar } from "../../src/components/StatusBar/StatusBar";

describe("StatusBar component", () => {
  it("displays online backend status when connected", () => {
    const mockInfo = {
      app_name: "linuxscp",
      app_version: "0.1.0",
      os_name: "linux",
      os_family: "unix",
      arch: "x86_64",
    };

    render(<StatusBar systemInfo={mockInfo} backendConnected={true} pingLatencyMs={12} />);

    expect(screen.getByText("Backend Online")).toBeInTheDocument();
    expect(screen.getByText("(12ms)")).toBeInTheDocument();
    expect(screen.getByText("linux (x86_64)")).toBeInTheDocument();
    expect(screen.getByText("Strict Host-Key Verification")).toBeInTheDocument();
  });

  it("displays disconnected indicator when backend is offline", () => {
    render(<StatusBar systemInfo={null} backendConnected={false} pingLatencyMs={null} />);

    expect(screen.getByText("Backend Disconnected")).toBeInTheDocument();
  });

  it("displays prominent ELEVATED / ROOT indicator when active session is elevated", () => {
    const mockSession = {
      session_id: "sess-1",
      profile_id: "prof-1",
      protocol: "SFTP" as const,
      host: "10.0.0.1",
      port: 22,
      username: "deploy",
      status: { status: "Connected" as const },
      capabilities: {
        protocol: "SFTP" as const,
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
      elevation: {
        type: "ElevatedSudo" as const,
        effective_user: "root",
        executed_command: "sudo -n /usr/lib/openssh/sftp-server",
      },
      current_remote_path: "/root",
      home_remote_path: "/home/deploy",
      connected_at_ms: Date.now(),
      last_active_ms: Date.now(),
    };

    render(
      <StatusBar
        systemInfo={null}
        backendConnected={true}
        pingLatencyMs={10}
        activeSession={mockSession}
      />
    );

    expect(screen.getByText(/ELEVATED \/ ROOT \(root\)/i)).toBeInTheDocument();
  });
});
