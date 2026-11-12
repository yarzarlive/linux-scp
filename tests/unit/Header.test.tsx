import { render, screen, fireEvent } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import { Header } from "../../src/components/Header/Header";

describe("Header component", () => {
  it("renders application name and version", () => {
    render(<Header appName="LinuxSCP" appVersion="0.1.0" />);
    expect(screen.getByText("LinuxSCP")).toBeInTheDocument();
    expect(screen.getByText("v0.1.0")).toBeInTheDocument();
    expect(screen.getByText("File")).toBeInTheDocument();
    expect(screen.getByText("Commands")).toBeInTheDocument();
  });

  it("opens Commands and Options menus for SSH terminal actions", () => {
    const handleOpenTerminal = vi.fn();
    const handleOpenSettings = vi.fn();

    render(
      <Header
        appName="LinuxSCP"
        appVersion="0.1.0"
        canOpenTerminal={true}
        onOpenTerminal={handleOpenTerminal}
        onOpenTerminalSettings={handleOpenSettings}
      />
    );

    fireEvent.click(screen.getByText("Commands"));
    const openTermItem = screen.getByRole("menuitem", { name: /Open in SSH Terminal/i });
    fireEvent.click(openTermItem);
    expect(handleOpenTerminal).toHaveBeenCalledTimes(1);

    fireEvent.click(screen.getByText("Options"));
    const settingsItem = screen.getByRole("menuitem", { name: /Terminal Settings/i });
    fireEvent.click(settingsItem);
    expect(handleOpenSettings).toHaveBeenCalledTimes(1);
  });
});
