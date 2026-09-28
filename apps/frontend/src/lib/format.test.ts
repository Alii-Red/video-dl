import { describe, expect, it } from "vitest";
import { formatEta, formatSpeed } from "./format";

describe("formatSpeed", () => {
  it("returns null for missing or non-positive speeds", () => {
    expect(formatSpeed(null)).toBeNull();
    expect(formatSpeed(undefined)).toBeNull();
    expect(formatSpeed(0)).toBeNull();
  });

  it("scales bytes/s to human units", () => {
    expect(formatSpeed(500)).toBe("500 o/s");
    expect(formatSpeed(1536)).toBe("1.5 Ko/s");
    expect(formatSpeed(5 * 1024 * 1024)).toBe("5.0 Mo/s");
  });
});

describe("formatEta", () => {
  it("returns null for missing/negative", () => {
    expect(formatEta(null)).toBeNull();
    expect(formatEta(-1)).toBeNull();
  });

  it("formats mm:ss under an hour", () => {
    expect(formatEta(0)).toBe("00:00");
    expect(formatEta(75)).toBe("01:15");
  });

  it("formats h:mm:ss past an hour", () => {
    expect(formatEta(3661)).toBe("1:01:01");
  });
});
