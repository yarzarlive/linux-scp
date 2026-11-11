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

    render(
      <StatusBar
        systemInfo={mockInfo}
        backendConnected={true}
        pingLatencyMs={12}
      />
    );

    expect(screen.getByText("Backend Online")).toBeInTheDocument();
    expect(screen.getByText("(12ms)")).toBeInTheDocument();
    expect(screen.getByText("linux (x86_64)")).toBeInTheDocument();
    expect(screen.getByText("Strict Host-Key Verification")).toBeInTheDocument();
  });

  it("displays disconnected indicator when backend is offline", () => {
    render(
      <StatusBar
        systemInfo={null}
        backendConnected={false}
        pingLatencyMs={null}
      />
    );

    expect(screen.getByText("Backend Disconnected")).toBeInTheDocument();
  });
});
