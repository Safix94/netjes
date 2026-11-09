import { afterEach, describe, expect, it, vi } from "vitest";
import { copy } from "../copy";
import { isMac } from "../lib/platform";

afterEach(() => vi.restoreAllMocks());

describe("platform fallback", () => {
  it.each(["Win32", "Linux x86_64", "", "unknown"])("keeps Ctrl and Verkenner on %s", (platform) => {
    vi.spyOn(navigator, "platform", "get").mockReturnValue(platform);
    expect(isMac()).toBe(false);
    expect(copy.recordingHint).toContain("Ctrl+Shift+R");
    expect(copy.reveal).toBe("Toon in Verkenner");
  });
});
