import { render, screen, fireEvent } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ConnectionToolbar } from "../../src/components/Toolbars/ConnectionToolbar";

describe("ConnectionToolbar Component", () => {
  it("renders toolbar actions and status", () => {
    const handleNewSession = vi.fn();
    const handleSiteManager = vi.fn();
    const handleDisconnect = vi.fn();

    render(
      <ConnectionToolbar
        isConnected={false}
        hostKeyVerified={true}
        onNewSession={handleNewSession}
        onSiteManager={handleSiteManager}
        onDisconnect={handleDisconnect}
      />
    );

    expect(screen.getByText("New Session")).toBeInTheDocument();
    expect(screen.getByText("Site Manager")).toBeInTheDocument();
    expect(screen.getByText("Strict Host-Key Verification")).toBeInTheDocument();
    expect(screen.getByText("Remote Disconnected")).toBeInTheDocument();

    const disconnectBtn = screen.getByRole("button", { name: /Disconnect/i });
    expect(disconnectBtn).toBeDisabled();

    fireEvent.click(screen.getByRole("button", { name: /New Session/i }));
    expect(handleNewSession).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /Site Manager/i }));
    expect(handleSiteManager).toHaveBeenCalledTimes(1);
  });

  it("enables disconnect button when connected", () => {
    const handleDisconnect = vi.fn();
    render(
      <ConnectionToolbar
        isConnected={true}
        hostKeyVerified={true}
        onDisconnect={handleDisconnect}
      />
    );

    const disconnectBtn = screen.getByRole("button", { name: /Disconnect/i });
    expect(disconnectBtn).not.toBeDisabled();
    expect(screen.getByText("Remote Connected")).toBeInTheDocument();

    fireEvent.click(disconnectBtn);
    expect(handleDisconnect).toHaveBeenCalledTimes(1);
  });

  it("renders Open Terminal action and enables it when a connection profile is selected", () => {
    const handleOpenTerminal = vi.fn();
    const handleOpenTerminalSettings = vi.fn();

    const { rerender } = render(
      <ConnectionToolbar
        isConnected={false}
        canOpenTerminal={false}
        onOpenTerminal={handleOpenTerminal}
        onOpenTerminalSettings={handleOpenTerminalSettings}
      />
    );

    const terminalBtn = screen.getByRole("button", { name: /Open Terminal/i });
    expect(terminalBtn).toBeDisabled();

    rerender(
      <ConnectionToolbar
        isConnected={false}
        canOpenTerminal={true}
        onOpenTerminal={handleOpenTerminal}
        onOpenTerminalSettings={handleOpenTerminalSettings}
      />
    );

    expect(screen.getByRole("button", { name: /Open Terminal/i })).not.toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: /Open Terminal/i }));
    expect(handleOpenTerminal).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByRole("button", { name: /Terminal Settings/i }));
    expect(handleOpenTerminalSettings).toHaveBeenCalledTimes(1);
  });
});
