import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { Header } from "../../src/components/Header/Header";

describe("Header component", () => {
  it("renders application name and version", () => {
    render(<Header appName="LinuxSCP" appVersion="0.1.0" />);
    expect(screen.getByText("LinuxSCP")).toBeInTheDocument();
    expect(screen.getByText("v0.1.0")).toBeInTheDocument();
    expect(screen.getByText("File")).toBeInTheDocument();
    expect(screen.getByText("Commands")).toBeInTheDocument();
  });
});

