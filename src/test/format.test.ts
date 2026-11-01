import { describe, expect, it } from "vitest";
import { countLabel } from "../lib/format";

describe("format", () => {
  it("countLabel", () => {
    expect(countLabel(1, "bestand", "bestanden")).toBe("1 bestand");
    expect(countLabel(431, "bestand", "bestanden")).toBe("431 bestanden");
    expect(countLabel(0, "bestand", "bestanden")).toBe("0 bestanden");
    expect(countLabel(12345, "bestand", "bestanden")).toBe("12345 bestanden");
  });
});
