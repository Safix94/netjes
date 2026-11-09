# Netjes voor macOS: goedgekeurd ontwerp

De gebruiker heeft op 9 november 2026 het onderzoek en de voorgestelde aanpak goedgekeurd en gevraagd het plan uit te voeren. Er is nog geen Apple Developer-account en geen eigen Mac. Dit document legt dat ontwerp vast; het is geen bewijs dat de publieke Mac-release al klaar is.

## Gewenst resultaat

Netjes blijft dezelfde lokale, gratis opruimapp, met dezelfde React-interface en Rust-logica. Een universele DMG bevat de app voor Apple Silicon en Intel. Gebruikers downloaden die via agentiek.be, slepen Netjes naar Programma's en geven macOS toestemming voor hun Downloads-map. Geen App Store-publicatie en geen instructies om beveiliging uit te schakelen.

## Bindende voorwaarden

- Bestaande Windows-functionaliteit en bestaande Windows-herstelgegevens blijven werken.
- Opruimen en ongedaan maken overschrijven nooit een bestaand doelbestand, ook niet bij een gelijktijdig naamconflict. Een niet-ondersteunde veilige verplaatsing faalt gesloten.
- Alleen gewone lokale bestanden in de gekozen map worden verwerkt. Submappen, symlinks, verborgen bestanden, onafgewerkte downloads en niet-lokale cloudbestanden blijven ongemoeid. macOS-pakketten die mappen zijn blijven dus ook ongemoeid.
- Schijfwortels, systeemmappen, de gebruikersmap zelf en de Library-map worden geweigerd. Alias- of symlinkpaden mogen die bescherming niet omzeilen.
- Hoofdlettergevoelige bestandssystemen mogen geen gedeelde herstelgegevens krijgen voor verschillende mappen.
- Miniaturen gebruiken een native Mac-voorziening, met begrensde wachttijd en veilige terugval naar de bestaande bestandstegel.
- Toegang weigeren levert een begrijpelijke Nederlandse melding met een herstelroute op; volledige schijftoegang is geen standaardvereiste.
- Bouwdoel macOS 13.0 of nieuwer, beide architecturen. CI draait op macos-15 en macos-15-intel. Compatibiliteit met de ondergrens blijft onderdeel van de praktische acceptatie.
- Publieke distributie vereist Developer ID-ondertekening, hardened runtime, succesvolle notarization en stapling. Zonder credentials geen publieke release.
- De huidige versiecheck blijft een melding met een downloadlink; geen automatische updater toevoegen.
- Geen telemetrie, nieuwe server, nieuwe componentbibliotheek of herontwerp. Bestaande UI-conventies blijven behouden.

## Distributie en acceptatie

GitHub CI bouwt en test de Mac-code en produceert een duidelijk als testbuild aangeduid universeel artifact. Een afzonderlijke handmatig gestarte releaseprocedure controleert secrets, bouwt, ondertekent, notariseert en verifieert het resultaat voordat een conceptrelease kan worden gemaakt. Die procedure publiceert niet automatisch.

Een echte browserdownload op een schone Mac moet voor publieke vrijgave bewijzen: installeren zonder beveiligingsomweg, eerste-startbevestiging, maptoegang toestaan/weigeren/herstellen, miniaturen, opruimen en ongedaan maken met naamconflicten, herstart/herstel, cloudbestanden, Finder en klein scherm. Test op Intel en Apple Silicon, en op de geclaimde minimale macOS-versie.

De websitebron staat niet in deze repository. Websitewijzigingen worden daarom als afzonderlijke vrijgavestap beschreven, niet als uitgevoerd gemeld. Apple-account, certificaten, notarization en menselijke Mac-acceptatie blijven expliciete externe voorwaarden zolang ze ontbreken.

## Bronnen

- https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution
- https://developer.apple.com/programs/enroll/
- https://v2.tauri.app/distribute/dmg/
- https://v2.tauri.app/distribute/sign/macos/
- https://docs.github.com/en/actions/how-tos/write-workflows/choose-where-workflows-run/choose-the-runner-for-a-job
