import { render } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FolderGroups } from "../components/FolderGroups";
import { CATEGORIES } from "./fixtures";

const props = (slotRefs: (cat: string, i: number, el: HTMLCanvasElement | null) => void) => ({
  categories: CATEGORIES,
  counts: { fotos: 3 },
  active: new Set<string>(["fotos"]),
  hit: new Set<string>(),
  slotRefs,
  onOpen: () => {},
});

describe("FolderGroups", () => {
  it("renders twelve 40x40 canvases per card and hands each one to slotRefs", () => {
    const slotRefs = vi.fn();
    const { container } = render(<FolderGroups {...props(slotRefs)} />);
    const canvases = container.querySelectorAll("canvas");
    expect(canvases).toHaveLength(CATEGORIES.length * 12);
    canvases.forEach((canvas) => {
      expect(canvas).toHaveAttribute("width", "40");
      expect(canvas).toHaveAttribute("height", "40");
    });
    expect(slotRefs).toHaveBeenCalledWith("fotos", 0, expect.any(HTMLCanvasElement));
    expect(slotRefs).toHaveBeenCalledWith("duplicaten", 11, expect.any(HTMLCanvasElement));
    expect(container.querySelector('[data-cat="duplicaten"]')).toHaveClass("wide");
    expect(container.querySelector('[data-cat="fotos"]')).toHaveClass("active");
  });

  it("keeps the canvases attached on re-render and releases them with null on unmount", () => {
    const slotRefs = vi.fn();
    const { rerender, unmount } = render(<FolderGroups {...props(slotRefs)} />);
    slotRefs.mockClear();
    rerender(<FolderGroups {...props(slotRefs)} />);
    expect(slotRefs).not.toHaveBeenCalled();
    unmount();
    expect(slotRefs).toHaveBeenCalledWith("fotos", 0, null);
    expect(slotRefs.mock.calls.filter(([, , el]) => el === null)).toHaveLength(CATEGORIES.length * 12);
  });
});
