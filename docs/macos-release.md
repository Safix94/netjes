# macOS: testbuild en voorbereiding op distributie

Netjes voor macOS is ontwikkelaarsklaar voor CI en acceptatie, maar **nog niet publiek vrijgegeven**. De universele testbuild bevat Apple Silicon (`arm64`) en Intel (`x86_64`), met bouwdoel macOS 13.0. CI draait op macOS 15; dit bewijst de werking op macOS 13 of echte gebruikershardware nog niet. Er is momenteel geen Apple Developer-account, certificaat of eigen test-Mac beschikbaar.

De versiecontrole blijft handmatig: de knop vergelijkt versies en opent een downloadlink. Er is geen automatische updater, telemetrie of nieuwe server toegevoegd.

## Universele testbuild

Op een Mac met Xcode Command Line Tools, Node 22, pnpm (de versie uit `package.json`) en Rust stable:

```sh
pnpm install --frozen-lockfile
rustup target add aarch64-apple-darwin x86_64-apple-darwin
export MACOSX_DEPLOYMENT_TARGET=13.0
export APPLE_SIGNING_IDENTITY=-
pnpm tauri build --target universal-apple-darwin --bundles app,dmg

bundle=src-tauri/target/universal-apple-darwin/release/bundle
images=("$bundle"/dmg/*.dmg)
bash scripts/verify-macos-bundle.sh --test "$bundle/macos/Netjes.app" "${images[0]}"
bash scripts/test-verify-macos-bundle.sh "$bundle/macos/Netjes.app" "${images[0]}"
```

Gebruik Bash voor de arraycommando's hierboven. De Objective-C Quick Look-bridge wordt door `cc` statisch voor elk Rust-target gebouwd. De CI controleert beide archieven, beide slices van de executable, frameworklinkage, de Mach-O-ondergrens en de gegenereerde plist.

Elke branch-CI bouwt zonder Apple-secrets en met alleen `contents: read` het artifact **Netjes-macOS-testbuild**. Download via de geslaagde run op [GitHub Actions](https://github.com/Safix94/netjes/actions/workflows/ci.yml), of met GitHub CLI:

```sh
gh run list --repo Safix94/netjes --branch feat/macos-distribution --workflow ci.yml
# Vul het run-ID van de gewenste commit in.
RUN_ID=123456789
gh run download "$RUN_ID" --repo Safix94/netjes --name Netjes-macOS-testbuild --dir artifacts/macos-testbuild
cd artifacts/macos-testbuild
shasum -a 256 -c SHA256SUMS
```

Het artifact bevat `Netjes-testbuild.dmg`, `Netjes.app.zip`, `SHA256SUMS`, `TESTBUILD.txt` met broncommit en `verification.txt`. Het verloopt na 30 dagen. De ZIP bewaart Mac-bundlestructuur en permissies. Pak hem op een Mac uit met `ditto -x -k Netjes.app.zip .` en voer de verificateur opnieuw uit tegen de gedownloade DMG. De verificateur mount de DMG alleen-lezen, vergelijkt de ingesloten app met de gecontroleerde app, controleert de Programma's-link en verifieert ook de gemounte app.

**Dit artifact is ad-hoc ondertekend en niet genotariseerd. Het is geen publieke installer.** De distributiemodus moet deze build weigeren:

```sh
bash scripts/verify-macos-bundle.sh --distribution Netjes.app Netjes-testbuild.dmg
# Verwacht: fout over Developer ID Application; nooit gebruiken als distributiegoedkeuring.
```

De CI-integratietest controleert ook ontbrekende argumenten/bestanden, een dunne executable, beschadigde handtekening, onjuiste minimale OS-versie en een geldige maar andere app dan die in de DMG. De releasepreflight heeft lokale tests: `python scripts/test-macos-release.py` (Python 3.11+).

## Voorwaarden voor een getekende conceptrelease

Voor distributie buiten de App Store zijn een [Apple Developer-lidmaatschap](https://developer.apple.com/programs/enroll/), een **Developer ID Application**-certificaat met private sleutel en App Store Connect-notarizationrechten nodig. Gebruik geen Apple Development- of App Store-distributieprofiel. Regel dit later via een privé secretkanaal; deel nooit sleutels of wachtwoorden in chat, Git, logs of artifacts.

Maak in GitHub de environment `macos-release`, bij voorkeur met verplichte reviewer en beperkingen tot vertrouwde release-refs. Configureer deze secretNAMEN (geen waarden in documentatie):

| Secret | Betekenis |
| --- | --- |
| `APPLE_CERTIFICATE` | Base64 van het geëxporteerde Developer ID Application `.p12`-certificaat inclusief private sleutel |
| `APPLE_CERTIFICATE_PASSWORD` | Wachtwoord van die `.p12`-export |
| `APPLE_SIGNING_IDENTITY` | Volledige identiteit `Developer ID Application: organisatienaam (TEAMID)` |
| `APPLE_TEAM_ID` | Het bijbehorende tien tekens lange Apple Team ID |
| `APPLE_API_KEY` | Tien tekens lange App Store Connect API-key-ID |
| `APPLE_API_ISSUER` | Issuer UUID van de App Store Connect-teamkey |
| `APPLE_API_PRIVATE_KEY` | Volledige inhoud van de bijbehorende `.p8`-private key |

Er is geen Apple ID-accountwachtwoord nodig. Het tijdelijke keychainwachtwoord wordt op de runner gegenereerd. Certificaat, API-key en keychain staan uitsluitend onder `$RUNNER_TEMP/netjes-signing`; een `always()`-stap herstelt de keychainzoeklijst en verwijdert het materiaal, ook bij mislukken of annuleren. Credentials worden niet als buildartifact opgeslagen.

## Handmatig een draft voorbereiden

De workflow `.github/workflows/macos-release.yml` is voorbereid; hij is zonder secrets niet uitgevoerd. GitHub vereist dat een `workflow_dispatch`-workflow eerst op de standaardbranch bestaat voordat deze handmatig gestart kan worden. Integratie in die branch en ondertekening zijn afzonderlijke toekomstige stappen, geen onderdeel van deze implementatie.

1. Rond de code- en CI-review af. Kies een **nieuwe** versie: een tag met een bestaande Windows-release wordt geweigerd, ook als deze draft is. Verhoog `package.json`, `src-tauri/Cargo.toml` (en de bijbehorende Cargo-lock packageversie) en `src-tauri/tauri.conf.json` samen in een gereviewde commit. De tag heeft exact de vorm `vMAJOR.MINOR.PATCH`.
2. Maak en push later expliciet die tag naar de bedoelde commit. Maak geen publieke release. Voorbeeld voor een toekomstige versie (pas versie/commit aan):

   ```sh
   TAG=v1.0.1
   SOURCE_SHA=0123456789abcdef0123456789abcdef01234567
   git tag -a "$TAG" "$SOURCE_SHA" -m "Netjes $TAG macOS candidate"
   git push origin "refs/tags/$TAG"
   gh workflow run macos-release.yml --repo Safix94/netjes --ref "$TAG" \
     -f "tag=$TAG" -f "source_sha=$SOURCE_SHA"
   gh run list --repo Safix94/netjes --workflow macos-release.yml
   ```

3. Controleer de run en de draft. Preflight vergelijkt checkout, bestaande tag, exacte SHA, alle drie versies, bestaande releases en alle secrets vóór de build. Invoer loopt via environmentvariabelen en gevalideerde argumenten; er worden geen onbewerkte workflow-inputs in shellcode geplakt.
4. De workflow importeert het certificaat in een tijdelijke keychain en bouwt universeel met hardened runtime. Tauri ondertekent en notariseert/staplet de app. De workflow ondertekent en notariseert/staplet vervolgens de DMG. De strikte verificateur controleert beide architecturen, Developer ID, team, runtime, timestamp, app- en DMG-tickets, Gatekeeper en de werkelijk ingesloten app. Fouten blokkeren de release.
5. Pas na verificatie én credentialopruiming maakt `gh release create --draft --latest=false --verify-tag` een **nieuwe conceptrelease** met stabiel asset `Netjes.dmg`. De workflow bewerkt nooit een bestaande release en vervangt geen Windows-assets. Een herstart met dezelfde draft-tag wordt bewust geweigerd; onderzoek de eerdere run voordat een nieuwe versie wordt gekozen. Publiceren blijft een menselijke stap na de acceptatie hieronder.

Op een geschikte Mac kan de strikte controle opnieuw met een niet-geheime team-ID:

```sh
APPLE_TEAM_ID=ABCDEFGHIJ bash scripts/verify-macos-bundle.sh --distribution Netjes.app Netjes.dmg
codesign --verify --deep --strict --all-architectures --verbose=2 Netjes.app
lipo -verify_arch arm64 x86_64 Netjes.app/Contents/MacOS/netjes
xcrun stapler validate Netjes.app
xcrun stapler validate Netjes.dmg
spctl --assess --type execute --verbose=2 Netjes.app
spctl --assess --type open --context context:primary-signature --verbose=2 Netjes.dmg
```

## Acceptatie vóór publieke vrijgave

Gebruik een schone Mac en een echte browserdownload van de **getekende, genotariseerde** kandidaat, zodat quarantine en Gatekeeper daadwerkelijk worden getest. Open de DMG, sleep Netjes naar Programma's, werp de DMG uit en start de geïnstalleerde app vanuit Finder. Bevestig alleen de normale eerste-startvraag. Een beveiligingsomweg is geen geslaagde installatie; bij een blokkade stopt de vrijgave en wordt de kandidaat onderzocht.

- [ ] Apple Silicon én Intel: browserdownload, DMG, slepen naar Programma's, Finder-start en eerste-startmelding zonder beveiligingsomweg.
- [ ] Dezelfde kernscenario's op echte macOS 13-systemen, inclusief beide architecturen voor zover die minimumclaim wordt gedaan; macOS 15-CI bewijst deze ondergrens niet.
- [ ] Downloads-toegang toestaan, weigeren en later herstellen met begrijpelijke Nederlandse melding en de juiste Instellingenroute. Volledige schijftoegang is geen standaardvereiste.
- [ ] Quick Look-miniaturen, niet-ondersteunde bestanden en timeout/terugval; groot bestandenaantal zonder vastlopende interface.
- [ ] Opruimen en ongedaan maken, dubbele namen en gelijktijdige naamconflicten zonder overschrijven; gedeeltelijke mislukking bewaart herstelmogelijkheid.
- [ ] Afsluiten/herstarten en herstel; hoofdlettergevoelig volume houdt herstelgegevens van verschillende mappen gescheiden.
- [ ] Geen verwerking van submappen/pakketten, symlinks/aliases als omweg, verborgen bestanden, onafgewerkte downloads of niet-lokale cloudbestanden.
- [ ] Schijfwortels, systeemmappen, gebruikersmap en Library blijven beschermd, ook via alias/symlinkpad.
- [ ] Finder openen, Command-sneltoetsen, klein scherm en minimale venstergrootte; bestaande handmatige versiecheck en downloadlink.
- [ ] Windows-regressie: installeren/starten, miniaturen, opruimen/ongedaan maken, bestaande herstelgegevens, Explorer, Control-sneltoetsen, handmatige versiecheck en de bestaande stabiele Windows-download.
- [ ] De eindkandidaat, SHA, artifacts en handmatige resultaten zijn vastgelegd en gereviewd; daarna pas expliciet besluiten over publicatie.

De websitebron voor **agentiek.be staat buiten deze repository**. Er is hier geen Mac-downloadknop toegevoegd. Publicatie en een websitewijziging gebeuren later vanuit de website-repository, na echte Mac-acceptatie, met behoud van de Windows-download. Deze workflow publiceert niets automatisch en wijzigt `latest` niet.

## Primaire documentatie

- [Tauri macOS-ondertekening, API-keynotarization en ad-hoc signing](https://v2.tauri.app/distribute/sign/macos/)
- [Tauri DMG](https://v2.tauri.app/distribute/dmg/)
- [Apple: notarizing macOS software before distribution](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
- [GitHub CLI: release create](https://cli.github.com/manual/gh_release_create)
- [Officiële Actions: checkout](https://github.com/actions/checkout), [setup-node](https://github.com/actions/setup-node), [upload-artifact](https://github.com/actions/upload-artifact), [pnpm/action-setup](https://github.com/pnpm/action-setup). De workflows pinnen de op 9 november 2026 gecontroleerde huidige majors aan exacte commits.
