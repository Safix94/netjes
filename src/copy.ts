// Alle zichtbare tekst van de app, in Belgisch Nederlands (je-vorm). Geen gedachtestreepjes,
// geen uitroeptekens, geen emoji. De test in src/test/copy.test.ts bewaakt dat.
import { countLabel } from "./lib/format";

const files = (n: number): string => countLabel(n, "bestand", "bestanden");

export const copy = {
  appName: "Netjes",
  eyebrow: "Netjes",
  titleBefore: "Mijn",
  titleAfter: "opruimen",
  /** De mapnaam in de titel zolang er nog geen map gescand is. */
  defaultFolder: "Downloads",
  statusScanning: "Even kijken…",
  statusReady: (n: number) => `Klaar · ${files(n)}`,
  statusTidy: "Al netjes",
  statusSorting: "Bezig met sorteren…",
  statusDone: "Klaar ✓ · klik een map om te kijken",
  statusUndoing: "Bezig met terugzetten…",
  organize: (n: number) => `${files(n)} opruimen`,
  undo: "Ongedaan maken",
  disorganize: "Rommelig maken",
  resort: "Opnieuw sorteren",
  chooseFolder: "Map kiezen",
  recording: "Opname",
  recordingHint: "Opnamemodus aan of uit (Ctrl+Shift+R)",
  capMess: "Een rommeltje",
  capTidy: "Opgeruimd",
  stats: {
    total: "Bestanden",
    photos: "Foto's",
    videos: "Video's",
    folders: "Mappen gemaakt",
    duplicates: "Duplicaten",
  },
  viewerMeta: (n: number) => `${files(n)} gesorteerd · klik om te openen`,
  close: "Sluiten",
  reveal: "Toon in Verkenner",
  confirmUndo: (n: number) =>
    `Zet ${files(n)} terug naar waar ze stonden voor de laatste opruimbeurt?`,
  confirmDisorganize:
    "Dit maakt de map opnieuw rommelig, enkel om het sorteren opnieuw te tonen. Doorgaan?",
  errorsTitle: (n: number) => `${files(n)} niet verplaatst`,
  moveFailed: "Verplaatsen mislukt.",
  noOutcome: "Geen antwoord gekregen voor dit bestand.",
  logNotSaved: "Het log voor Ongedaan maken kon niet bewaard worden. Deze opruimbeurt kan je niet terugzetten.",
  checkUpdate: "Controleren op updates",
  updateChecking: "Even kijken…",
  updateAvailable: (v: string) => `Nieuwe versie ${v} beschikbaar`,
  updateLatest: "Je hebt de nieuwste versie",
  updateError: "Kon niet controleren",
  updateOpen: "Download de nieuwe versie",
  forbiddenRoot: "Die map ruimt Netjes niet op. Kies een gewone map zoals Downloads.",
  // De zinnen die de Rust-kant zelf als fout teruggeeft; src/lib/errors.ts toont enkel deze.
  notAFolder: "Dit is geen map.",
  noDownloads: "Downloads-map niet gevonden",
  rescanFirst: "Scan de map opnieuw.",
  genericError: "Er ging iets mis. Probeer het opnieuw.",
};
