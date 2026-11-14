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

  it("toggles dark and light mode from Options menu", () => {
    const handleToggleTheme = vi.fn();

    const { rerender } = render(
      <Header
        appName="LinuxSCP"
        appVersion="0.1.0"
        isDarkMode={true}
        onToggleTheme={handleToggleTheme}
      />
    );

    // Open Options menu in dark mode
    fireEvent.click(screen.getByText("Options"));
    expect(screen.getByText("Appearance")).toBeInTheDocument();

    const lightModeBtn = screen.getByTitle("Switch to Light Mode");
    expect(lightModeBtn).toBeInTheDocument();
    expect(screen.getByText("Light Mode")).toBeInTheDocument();
    expect(screen.getByText("Dark Mode")).toBeInTheDocument();

    fireEvent.click(lightModeBtn);
    expect(handleToggleTheme).toHaveBeenCalledTimes(1);

    // Re-render in light mode
    rerender(
      <Header
        appName="LinuxSCP"
        appVersion="0.1.0"
        isDarkMode={false}
        onToggleTheme={handleToggleTheme}
      />
    );

    fireEvent.click(screen.getByText("Options"));
    const darkModeBtn = screen.getByTitle("Switch to Dark Mode");
    expect(darkModeBtn).toBeInTheDocument();

    fireEvent.click(darkModeBtn);
    expect(handleToggleTheme).toHaveBeenCalledTimes(2);
  });
});
