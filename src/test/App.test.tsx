import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import { copy } from "../copy";
import * as api from "../lib/api";
import { CATEGORIES, file } from "./fixtures";

vi.mock("../lib/api", () => ({
  getCategories: vi.fn(),
  getDownloadsDir: vi.fn(),
  scan: vi.fn(),
  checkUndo: vi.fn(),
  undoOrganize: vi.fn(),
  chooseFolder: vi.fn(),
  confirm: vi.fn(),
  revealInExplorer: vi.fn(),
}));

const ROOT = "C:\\Users\\Test\\Downloads";

const scanned = (root: string, ...categories: api.Category[]): api.ScanResult => ({
  root,
  files: categories.map(file),
  existing_category_folders: ["fotos"],
  skipped: 0,
});

const button = (name: string | RegExp) => screen.getByRole("button", { name });

beforeEach(() => {
  vi.mocked(api.getCategories).mockResolvedValue(CATEGORIES);
  vi.mocked(api.getDownloadsDir).mockResolvedValue(ROOT);
  vi.mocked(api.scan).mockImplementation(async (root) => scanned(root, "fotos", "fotos", "videos", "duplicaten"));
  vi.mocked(api.checkUndo).mockResolvedValue(null);
  vi.mocked(api.revealInExplorer).mockResolvedValue();
});

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("App", () => {
  it("scans the Downloads folder on start and shows the real numbers", async () => {
    render(<App />);
    expect(await screen.findByText(copy.statusReady(4))).toBeInTheDocument();
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("Mijn Downloads opruimen");
    expect(screen.getByRole("img", { name: "4" })).toBeInTheDocument(); // bestanden
    expect(screen.getByRole("img", { name: "2" })).toBeInTheDocument(); // foto's
    expect(button(copy.organize(4))).toBeEnabled();
    expect(button(copy.undo)).toBeDisabled();
  });

  it("calls an empty folder tidy and does not offer to organize it", async () => {
    vi.mocked(api.scan).mockImplementation(async (root) => scanned(root));
    render(<App />);
    expect(await screen.findByText(copy.statusTidy)).toBeInTheDocument();
    expect(button(copy.organize(0))).toBeDisabled();
  });

  it("shows a refused folder in the status and keeps Map kiezen available", async () => {
    vi.mocked(api.scan).mockRejectedValue("Die map ruimt Netjes niet op. Kies een gewone map zoals Downloads.");
    render(<App />);
    expect(await screen.findByText(copy.forbiddenRoot)).toBeInTheDocument();
    expect(button(copy.organize(0))).toBeDisabled();
    expect(button(copy.chooseFolder)).toBeEnabled();
  });

  it("keeps the previous folder when a newly chosen folder is refused", async () => {
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    vi.mocked(api.chooseFolder).mockResolvedValue("C:\\Windows");
    vi.mocked(api.scan).mockRejectedValue("Die map ruimt Netjes niet op. Kies een gewone map zoals Downloads.");
    fireEvent.click(button(copy.chooseFolder));
    expect(await screen.findByText(copy.forbiddenRoot)).toBeInTheDocument();
    expect(api.scan).toHaveBeenLastCalledWith("C:\\Windows");
    expect(screen.getByTitle(ROOT)).toBeInTheDocument();
    expect(button(copy.organize(4))).toBeEnabled();
  });

  it("opens a category folder in Verkenner only when it exists", async () => {
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    fireEvent.click(document.querySelector('[data-cat="audio"]')!);
    expect(api.revealInExplorer).not.toHaveBeenCalled();
    fireEvent.click(document.querySelector('[data-cat="fotos"]')!);
    expect(api.revealInExplorer).toHaveBeenCalledWith(`${ROOT}\\Foto's`);
  });

  it("asks before undoing, then restores and scans again", async () => {
    vi.mocked(api.checkUndo).mockResolvedValue({ count: 5, created_at_ms: 1 });
    vi.mocked(api.confirm).mockResolvedValue(true);
    vi.mocked(api.undoOrganize).mockResolvedValue({ restored: 5, errors: [] });
    render(<App />);
    await waitFor(() => expect(button(copy.undo)).toBeEnabled());
    fireEvent.click(button(copy.undo));
    await waitFor(() => expect(api.undoOrganize).toHaveBeenCalledWith(ROOT));
    expect(api.confirm).toHaveBeenCalledWith(copy.confirmUndo(5), copy.appName);
    await waitFor(() => expect(api.scan).toHaveBeenCalledTimes(2));
  });

  it("toggles recording mode with Ctrl+Shift+R and with the Opname button", async () => {
    render(<App />);
    await screen.findByText(copy.statusReady(4));
    expect(screen.queryByText(copy.disorganize)).not.toBeInTheDocument();
    fireEvent.keyDown(window, { key: "R", ctrlKey: true, shiftKey: true });
    expect(await screen.findByText(copy.disorganize)).toBeInTheDocument();
    expect(screen.getByText(copy.resort)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: copy.recording }));
    expect(screen.queryByText(copy.disorganize)).not.toBeInTheDocument();
  });
});
