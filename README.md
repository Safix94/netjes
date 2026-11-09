# Netjes

Netjes ruimt je Downloads-map op. Het is een gratis app van Agentiek voor Windows en macOS: je ziet je bestanden in nette mappen terechtkomen, met een animatie erbij. De Mac-versie is voorlopig een testversie.

- Netjes verplaatst enkel bestanden en verwijdert nooit iets.
- Alles draait volledig op je eigen pc. Er verlaat niets je computer.
- Je kan de opruimbeurt ongedaan maken: alles gaat terug naar waar het stond.

Meer uitleg volgt op agentiek.be.

## Download

Download rechtstreeks: https://github.com/Safix94/netjes/releases/latest/download/Netjes-Setup.exe. Die link geeft altijd de nieuwste versie. Het bestand heet `Netjes-Setup.exe` en werkt op Windows 10 en 11 (64 bit). Versie 1.0 installeert geen updates zelf: de knop "Controleren op updates" in de kop vergelijkt met de nieuwste release en linkt naar de downloadpagina. Geen telemetrie.

## Mac-versie — gratis, zonder Apple-account

**[Download de Mac-testversie (Intel en Apple Silicon)](https://github.com/Safix94/netjes/actions/runs/42657236671/artifacts/13695105932)**. GitHub vraagt hiervoor een GitHub-login. Pak het ZIP-bestand uit en open `Netjes-testbuild.dmg`. Dit Actions-artifact vervalt op 9 december 2026; het is nog geen blijvende download bij Releases.

De app bevat beide Mac-architecturen, met bouwdoel macOS 13 of nieuwer. De automatische tests draaien op macOS 15; installatie op een echte Mac en compatibiliteit met macOS 13 zijn nog niet handmatig bevestigd.

1. Open de DMG en sleep **Netjes** naar **Programma's**.
2. Open Netjes vanuit Programma's. De app is ad-hoc ondertekend, zonder Apple Developer ID of notarization. macOS kan hem daarom blokkeren.
3. Vertrouw je deze download, open dan **Systeeminstellingen → Privacy en beveiliging** en kies **Open toch** bij de melding over Netjes. Bevestig het openen. Die optie is niet op elke beheerde Mac beschikbaar; installatie zonder extra stappen is niet gegarandeerd. Zie [Apples uitleg](https://support.apple.com/nl-nl/102445).
4. Geef Netjes toegang tot de map die je wilt opruimen. Probeer de testversie eerst met een map met kopieën van bestanden.

Er is geen betaald Apple-account nodig voor deze distributieroute, ook niet voor de maker. De code staat op [de Mac-branch](https://github.com/Safix94/netjes/tree/feat/macos-distribution), met [pull request #1](https://github.com/Safix94/netjes/pull/1). De [bouw- en releasehandleiding](docs/macos-release.md) beschrijft de gratis route en de resterende praktijktests. De websitebron voor agentiek.be staat buiten deze repository; daar is nog geen Mac-downloadknop toegevoegd.

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
