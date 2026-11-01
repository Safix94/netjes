import { describe, expect, it } from "vitest";
import { copy } from "../copy";
import { errorText } from "../lib/errors";

describe("errorText", () => {
  it("passes the known Dutch messages of the Rust side through", () => {
    for (const message of [copy.forbiddenRoot, copy.notAFolder, copy.noDownloads, copy.rescanFirst]) {
      expect(errorText(message)).toBe(message);
    }
  });

  it("replaces everything else with the generic Dutch message", () => {
    expect(errorText("Access is denied. (os error 5)")).toBe(copy.genericError);
    expect(errorText("de map kon niet gelezen worden: toegang geweigerd")).toBe(copy.genericError);
    expect(errorText(new Error("failed to open path"))).toBe(copy.genericError);
    expect(errorText(undefined)).toBe(copy.genericError);
  });
});
