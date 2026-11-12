import { describe, expect, it } from "vitest";
import {
  bitsToMode,
  bitsToOctalString,
  bitsToSymbolicString,
  modeToBits,
  parseOctalInput,
} from "../../src/utils/permissions";

describe("permissions utilities", () => {
  it("converts standard modes and special bits between mode, octal, and symbolic strings", () => {
    const b755 = modeToBits(0o0755);
    expect(bitsToMode(b755)).toBe(0o0755);
    expect(bitsToOctalString(b755)).toBe("0755");
    expect(bitsToSymbolicString(b755)).toBe("rwxr-xr-x");

    const b4755 = modeToBits(0o4755);
    expect(b4755.setUid).toBe(true);
    expect(b4755.setGid).toBe(false);
    expect(b4755.sticky).toBe(false);
    expect(bitsToOctalString(b4755)).toBe("4755");
    expect(bitsToSymbolicString(b4755)).toBe("rwsr-xr-x");

    const b1777 = modeToBits(0o1777);
    expect(b1777.sticky).toBe(true);
    expect(bitsToSymbolicString(b1777)).toBe("rwxrwxrwt");

    const b7000 = modeToBits(0o7000);
    expect(bitsToSymbolicString(b7000)).toBe("--S--S--T");
  });

  it("validates 3- and 4-digit octal input and rejects invalid values", () => {
    const valid3 = parseOctalInput("644");
    expect(valid3.valid).toBe(true);
    if (valid3.valid) {
      expect(valid3.mode).toBe(0o644);
    }

    const valid4 = parseOctalInput("2750");
    expect(valid4.valid).toBe(true);
    if (valid4.valid) {
      expect(valid4.bits.setGid).toBe(true);
    }

    expect(parseOctalInput("").valid).toBe(false);
    expect(parseOctalInput("789").valid).toBe(false);
    expect(parseOctalInput("07777").valid).toBe(false);
    expect(parseOctalInput("rwx").valid).toBe(false);
  });
});
