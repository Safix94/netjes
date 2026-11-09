# Netjes

Netjes ruimt je Downloads-map op. Het is een gratis Windows-app van Agentiek: je ziet je bestanden in nette mappen terechtkomen, met een animatie erbij.

- Netjes verplaatst enkel bestanden en verwijdert nooit iets.
- Alles draait volledig op je eigen pc. Er verlaat niets je computer.
- Je kan de opruimbeurt ongedaan maken: alles gaat terug naar waar het stond.

Meer uitleg volgt op agentiek.be.

## Download

Download rechtstreeks: https://github.com/Safix94/netjes/releases/latest/download/Netjes-Setup.exe. Die link geeft altijd de nieuwste versie. Het bestand heet `Netjes-Setup.exe` en werkt op Windows 10 en 11 (64 bit). Versie 1.0 installeert geen updates zelf: de knop "Controleren op updates" in de kop vergelijkt met de nieuwste release en linkt naar de downloadpagina. Geen telemetrie.

## macOS in voorbereiding

De Mac-versie is nog niet publiek vrijgegeven. CI bouwt een universele testbuild voor Apple Silicon en Intel met bouwdoel macOS 13.0. Het artifact `Netjes-macOS-testbuild` op [GitHub Actions](https://github.com/Safix94/netjes/actions/workflows/ci.yml) is ad-hoc ondertekend en niet genotariseerd: het is geen publieke installer.

[Bouw- en releasehandleiding](docs/macos-release.md) bevat de commando's, verificatie en acceptatiechecklist. Publieke distributie wacht op een Apple Developer-account, Developer ID-ondertekening, notarization en tests op echte Macs, inclusief macOS 13. De voorbereide releaseworkflow maakt uitsluitend een conceptrelease. De websitebron voor agentiek.be staat buiten deze repository; een Mac-downloadknop is een latere vrijgavestap.

## Windows-waarschuwing

Windows SmartScreen kan een waarschuwing tonen omdat het installatiebestand nog niet ondertekend is: klik op "Meer informatie" en daarna op "Toch uitvoeren". Houdt je browser de download tegen, kies dan "Behouden".

## Wat er met je bestanden gebeurt

- Netjes bekijkt enkel de losse bestanden in de map zelf. Submappen, snelkoppelingen, verborgen en tijdelijke bestanden blijven met rust.
- Elk bestand gaat naar een categoriemap in dezelfde map: Foto's, Screenshots, GIF's, Design, Video's, Schermopnames, PDF's, Documenten, Rekenbladen, Presentaties, Audio, Archieven, Installers, Code, Overig en Duplicaten.
- Netjes overschrijft nooit iets. Bestaat er in de doelmap al een bestand met dezelfde naam, dan krijgt het verplaatste bestand een vrije naam.
- Bestanden met exact dezelfde inhoud herkent Netjes als duplicaten. Het oudste bestand blijft het origineel en gaat naar zijn gewone categoriemap, de kopieën gaan naar Duplicaten.
- Met "Ongedaan maken" zet je alles terug naar waar het stond voor de laatste opruimbeurt. Enkel de lege mappen die Netjes zelf aanmaakte, verdwijnen dan weer.

## Zelf bouwen

Vereisten:

- Node 22 en pnpm
- Rust stable
- Microsoft C++ Build Tools, met de workload "Desktop development with C++"
- WebView2, dat standaard op Windows 10 en 11 staat

Commando's:

```
pnpm install
pnpm tauri dev
pnpm tauri build
```

De installer komt dan in `src-tauri/target/release/bundle/nsis/`.

Tests:

```
pnpm test
cargo test --manifest-path src-tauri/Cargo.toml
```

Oefenen zonder je echte Downloads-map: `.\scripts\maak-testmap.ps1` maakt `Netjes-testmap` op je bureaublad met 1.000 bestanden (600 PNG's, 100 MP4's, 300 documenten, 8 duplicaten). Kies die map in Netjes met "Map kiezen". Opnieuw vullen kan met `-Force`.

De animatie zonder Tauri bekijken: `pnpm dev` en dan http://localhost:1420/harness.html?n=1000 (nepbestanden, met fps-teller en pauzeknop).

## Licentie

MIT, Stinis Media CommV. Zie `LICENSE`.
