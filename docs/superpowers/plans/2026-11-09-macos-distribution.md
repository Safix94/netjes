# Netjes macOS Implementation Plan

**Bijgesteld besluit na implementatie:** de gebruiker kiest gratis distributie zonder betaald Apple-account. De hieronder afgevinkte taken beschrijven de uitgevoerde bouw en controles; de getekende workflow is voortaan optioneel. De actuele download en installatie-uitleg staan in de README en [releasehandleiding](../../macos-release.md). Een echte Mac-installatietest blijft open, maar het ontbreken van Apple-credentials blokkeert de gratis route niet.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox syntax for tracking.

**Goal:** Een veilige Mac-versie bouwen en technisch controleren, met een voorbereide rechtstreekse distributie buiten de App Store.

**Architecture:** Behoud Tauri 2, React en Rust. Isoleer besturingssysteemgedrag met cfg en een kleine native Mac-miniatuurbridge; behoud de bestaande commando-interfaces. Gebruik afzonderlijke platformconfiguratie, CI op beide Mac-architecturen en een gesloten releaseprocedure.

**Tech Stack:** Rust stable, Tauri 2, React 19, TypeScript, pnpm 10.20.0, Node 22, GitHub Actions, Apple Quick Look.

**Spec:** `docs/superpowers/specs/2026-11-09-macos-distribution.md`

## Global Constraints

- Bestaande Windows-functionaliteit en bestaande Windows-herstelgegevens blijven werken.
- Opruimen en ongedaan maken overschrijven nooit een bestaand doelbestand, ook niet bij een gelijktijdig naamconflict. Een niet-ondersteunde veilige verplaatsing faalt gesloten.
- Alleen gewone lokale bestanden in de gekozen map worden verwerkt. Submappen, symlinks, verborgen bestanden, onafgewerkte downloads en niet-lokale cloudbestanden blijven ongemoeid. macOS-pakketten die mappen zijn blijven dus ook ongemoeid.
- Schijfwortels, systeemmappen, de gebruikersmap zelf en de Library-map worden geweigerd. Alias- of symlinkpaden mogen die bescherming niet omzeilen.
- Hoofdlettergevoelige bestandssystemen mogen geen gedeelde herstelgegevens krijgen voor verschillende mappen.
- Miniaturen gebruiken een native Mac-voorziening, met begrensde wachttijd en veilige terugval naar de bestaande bestandstegel.
- Toegang weigeren levert een begrijpelijke Nederlandse melding met een herstelroute op; volledige schijftoegang is geen standaardvereiste.
- Bouwdoel macOS 13.0 of nieuwer, beide architecturen. CI draait op macos-15 en macos-15-intel. Compatibiliteit met de ondergrens blijft onderdeel van de praktische acceptatie.
- De gekozen publieke distributie gebruikt ad-hoc ondertekening zonder betaald Apple-account, met duidelijke uitleg over Gatekeeper en Open toch. Developer ID/notarization is optioneel.
- De huidige versiecheck blijft een melding met een downloadlink; geen automatische updater toevoegen.
- Geen telemetrie, nieuwe server, nieuwe componentbibliotheek of herontwerp. Bestaande UI-conventies blijven behouden.

## Task 1: Veilige platformlogica en Mac-testomgeving

**Files:** `src-tauri/src/{mover,paths,commands,undo,scan,planner}.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `.github/workflows/ci.yml`, `src-tauri/tauri.macos.conf.json`, tests in die Rust-modules.

**Interfaces:** Behoud alle bestaande Tauri-commandosignaturen. `move_path_no_replace` blijft het gedeelde verplaatsingspad voor opruimen en herstel. De scan levert alleen verwerkbare lokale gewone bestanden; `skipped` telt uitgesloten bestanden. `normalize_root` levert een consistent absoluut platformpad; Mac-paden behouden hoofdletters.

- [x] Maak regressietests voor weigeren van bestaand doel (bestaande Windows-only test ook op Mac), Unix-root, beschermde/gesymlinkte mappen, verschillende herstelbestanden bij hoofdletterverschil, cloud-/verborgen bestanden en pakketmappen. Platformtests gebruiken cfg; tests mogen geen echte gebruikersbestanden verplaatsen.

```rust
let dir = tempfile::tempdir().unwrap();
let src = dir.path().join("nieuw.txt");
let dst = dir.path().join("doel.txt");
std::fs::write(&src, b"nieuw").unwrap();
std::fs::write(&dst, b"oud").unwrap();
assert_eq!(rename_no_replace(&src, &dst).unwrap_err().kind(), std::io::ErrorKind::AlreadyExists);
assert_eq!(std::fs::read(&src).unwrap(), b"nieuw");
assert_eq!(std::fs::read(&dst).unwrap(), b"oud");
```

- [x] Draai gerichte tests voor implementatie en leg RED vast waar dit platform dat toelaat. Mac-only RED wordt op de Mac-runner gecontroleerd; rapporteer geen niet-uitgevoerde test.
- [x] Gebruik op Mac `libc::renamex_np` met `libc::RENAME_EXCL` en CString uit native Unix-padbytes. Geef systeemfouten terug, zonder terugval op overschrijvende rename. Windows MoveFileExW blijft behouden. Andere niet-ondersteunde platformen falen veilig.
- [x] Splits padnormalisatie en systeemmapbescherming per platform. Behoud `/`, canonicaliseer bestaande Mac-mappen, controleer wortels/mountpoints en beschermde bomen inclusief home/Library. Behoud Windows-herstelhashes; gebruik op Mac hoofdletterbehoudende consistente paden.
- [x] Behandel Mac hidden/dataless flags, puntbestanden en onvolledige downloads. Sla niet-lokale bestanden over voordat hashen, miniaturen of verplaatsen begint. Controleer `is_file()` zodat speciale bestanden buiten de scan blijven. Voeg alleen macOS-afhankelijkheden onder de macOS-cfg toe.
- [x] Voeg macos-15 en macos-15-intel toe aan CI met bestaande tests/build/clippy. Voeg minimale Mac-config toe: app/dmg, minimumSystemVersion 13.0; Windows NSIS blijft zoals het is. Gebruik geen Apple-secrets in PR-CI.
- [x] Draai `cargo test --manifest-path src-tauri/Cargo.toml` en `cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings` op Windows; commit de taak. Controller pusht de featurebranch en verzamelt dezelfde resultaten van beide Mac-runners, en laat fouten oplossen voor afronding.

## Task 2: Native miniaturen en bruikbare Mac-interface

**Files:** `src-tauri/src/thumbs.rs`, nieuwe `src-tauri/native/macos_thumbnails.m` indien nodig, `src-tauri/build.rs`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, `src-tauri/Info.plist`, `src-tauri/src/{commands,scan}.rs`, `src/{App.tsx,copy.ts}`, `src/lib/{errors,platform}.ts`, `src/styles/app.css`, Mac-config, relevante `src/test` tests.

**Interfaces:** Bestaande `get_thumbnails(paths, px)` blijft een geordende lijst cachepaden/null geven. Task 1 levert een veilige scan. De native bridge is uitsluitend een interne thumbnailfunctie; mislukking geeft None en mag de opruimactie nooit blokkeren. UI-platformherkenning wordt centraal afgeleid, met veilige Windows-/browserfallback.

- [x] Voeg tests toe voor echte PNG-miniaturen/cache op Mac, ongeldige invoer en fallback; voor Cmd+Shift+R op Mac en Ctrl+Shift+R op Windows; voor begrijpelijke toestemmingweigering.

```typescript
expect(errorText("Geen toegang tot deze map. Geef Netjes toegang via Systeeminstellingen > Privacy en beveiliging > Bestanden en mappen en probeer opnieuw."))
  .toContain("Systeeminstellingen");
```

- [x] Draai de relevante tests voor de wijziging en leg het ontbrekende gedrag vast. Gebruik voor native tests de Mac-CI uit Task 1.
- [x] Voeg Quick Look-thumbnailgeneratie toe. Voorkeur: kleine Objective-C bridge via `cc` in build.rs, gelinkt aan Foundation/AppKit/QuickLookThumbnailing en gecompileerd met ARC, zonder los uitvoerbaar hulpmiddel. Rust behoudt de bestaande PNG-cache. Begrens aanvraagwachttijd; behandel timeout, callback-lifetime, geheugenownership en foutpaden veilig. Gebruik geen shellinterpolatie of netwerk.
- [x] Laat miniaturen alleen gewone lokale bestanden lezen en behoud de bestaande bestandstegel bij onbekende typen. Test PNG-inhoud en cachehergebruik, geen assertion op alleen het bestaan van een mock.
- [x] Voeg Nederlandse Downloads/Desktop/Documents-gebruiksbeschrijvingen toe waar vereist. Vertaal permission denied/operation not permitted naar stabiele, begrijpelijke fouttekst en behoud die door de UI-errorfilter. Geef de gebruiker een herstelroute met opnieuw proberen/map kiezen; vraag geen volledige schijftoegang.
- [x] Gebruik Finder-tekst en Command-toets op Mac, behoud Windows-tekst en Ctrl. Gebruik de bestaande Tauri opener/dialog. Maak venster en raster bruikbaar op kleinere MacBook-schermen met beperkte CSS/configwijzigingen; geen herontwerp of componentmigratie.
- [x] Draai gerichte UI-tests, daarna `pnpm test`, `pnpm build`, Rust-tests/clippy. Verhelp uitsluitend de reeds bekende canvas-testwaarschuwing als de aanpassing dezelfde tests raakt. Commit en rapporteer resultaten/voorwaarden voor Mac-CI.

## Task 3: Universele testbuild en gecontroleerde distributie

**Files:** `.github/workflows/ci.yml`, nieuwe `.github/workflows/macos-release.yml`, nieuwe `scripts/verify-macos-bundle.sh`, nieuwe `docs/macos-release.md`, `README.md`, eventuele uitsluitend voor release nodige Tauri-config.

**Interfaces:** Consumes: geteste app en Mac-config uit Tasks 1-2. Produces: universele DMG als CI-testartifact en ondertekende/notarized DMG uitsluitend als conceptrelease na expliciete releaseworkflow. Geen wijziging aan de werking van de versiecheck.

- [x] Voeg op de arm64 Mac-runner een universele build toe, beide Rust-targets en eventueel ad-hoc signing alleen voor het testartifact. Gebruik `pnpm tauri build --target universal-apple-darwin --bundles app,dmg`. Benoem artifact duidelijk `Netjes-macOS-testbuild` en controleer de architecturen met `lipo -archs`. Upload pas na succesvolle build; dit is geen publieke installer.
- [x] Maak een handmatig te starten releaseworkflow met minimale rechten en een expliciet ref/tag. Controleer versie/tagconsistentie en vereiste secrets voordat gebouwd wordt. Gebruik Developer ID Application, geen App Store-profiel. Onderteken met hardened runtime; laat Tauri notarization uitvoeren, controleer/staple app en DMG. Bewaar certificaat/API-key alleen in tijdelijke runnerbestanden en ruim die met always op.
- [x] Voeg een echte bundle-verificatie toe die faalt bij ontbrekende architectuur, ongeldige handtekening of ontbrekend notarization-ticket. De publieke stap gebruikt onder andere:

```sh
codesign --verify --deep --strict --verbose=2 "$app"
xcrun stapler validate "$app"
xcrun stapler validate "$dmg"
spctl --assess --type execute --verbose=2 "$app"
lipo -verify_arch arm64 x86_64 "$app/Contents/MacOS/netjes"
```

- [x] Test de verificateur op de werkelijke CI-app (testmodus controleert structuur/architecturen; distributiemodus moet de niet-genotariseerde build weigeren). Test ontbrekende argumenten/bestanden, zonder de broncode als test te greppen.
- [x] Maak hoogstens een draft release met het stabiele asset `Netjes.dmg`, nooit automatisch public/latest. Bestaande Windows-releases/assets mogen niet worden vervangen. De definitieve publicatiestap blijft handmatig na Mac-acceptatie.
- [x] Documenteer bouwen, Apple-accountvereisten, benodigde secretNAMEN (geen waarden), releasecommando, downloads, veilige installatie en de checklist voor echte Macs. README maakt onderscheid tussen Windows-download en nog niet vrijgegeven Mac-ondersteuning. Geef precies aan dat de websitebron buiten deze repository staat.
- [x] Draai lokale suites en Mac-CI op deze branch. Verifieer de opgeslagen universele DMG, bewaar de run/artifactverwijzingen in het rapport. Laat code review uitvoeren en los concrete findings op. Publieke ondertekening en acceptatie blijven open zolang gebruiker geen account/Mac heeft.

## Externe vrijgavevoorwaarden

- Niet vereist voor de gekozen route: Apple Developer-lidmaatschap, certificaat en uitvoering van de optionele getekende releaseworkflow.
- [ ] Browserdownload en acceptatie op echte Macs, inclusief macOS 13-ondergrens, afronden.
- [ ] Na acceptatie release publiceren en Mac-downloadknop op agentiek.be toevoegen vanuit de website-repository.
