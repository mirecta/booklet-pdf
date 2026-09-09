# Booklet PDF

Multiplatformná aplikácia (Rust + GTK4), ktorá z bežného PDF vyrobí PDF
pripravené na tlač knihy: **2 strany na jeden list na ležato**, v správnom
poradí pre skladanie a šitie.

- `booklet-gui` – grafická aplikácia s náhľadom rozloženia vrátane obsahu strán
- `booklet` – rovnaká funkcionalita z príkazovej riadky
- `booklet-core` – knižnica s impozíciou (bez GTK, dá sa použiť samostatne)

Obsah strán sa neprekódováva: každá zdrojová strana sa zabalí do Form
XObjectu, takže fonty, vektory aj obrázky zostanú v pôvodnej kvalite.

Všetko je čistý Rust — žiadne C knižnice ani externé programy. Náhľady
rasterizuje [hayro](https://github.com/LaurenzV/hayro), impozíciu robí
[lopdf](https://github.com/J-F-Liu/lopdf).

## Režimy skladania

### Brožúra – zošitá v strede (saddle stitch)

Celý dokument je jeden zošit. Na prvom liste je zvonku posledná a prvá strana,
takže po naskladaní listov na seba a zošití v strede vznikne brožúra
s obálkou vpredu aj vzadu.

Príklad pre 8 strán (list = jeden papier, líce/rub = strany papiera):

| list | líce (vľavo \| vpravo) | rub (vľavo \| vpravo) |
|------|------------------------|-----------------------|
| 1    | 8 \| 1                 | 2 \| 7                |
| 2    | 6 \| 3                 | 4 \| 5                |

### Zošity – skladané po častiach (signatúry)

Dokument sa rozdelí na zošity po N listoch (N × 4 strany). Každý zošit sa
poskladá a zošije zvlášť, zošity sa potom zošijú alebo zlepia za sebou. Toto je
spôsob, akým sa vyrábajú skutočné knihy — brožúra so 200 stranami by mala
nepoužiteľne veľký presah v prehybe.

Pozor na jednu vec: pri predvolených 4 listoch v zošite (16 strán) dá dokument
so 16 alebo menej stranami **rovnaký výsledok ako brožúra** — celý sa zmestí do
jedného zošita. Aplikácia to napíše do súhrnu pod nastaveniami. Rozdelenie sa
prejaví až od 17. strany:

```
$ booklet kniha.pdf --mode signatures -n 4 --dry-run
40 zdrojových strán, 10 listov, 20 strán výstupu, 0 prázdnych miest
3 zošity, listov po 4+4+2
  zošit 1: strany 1–16    zošit 2: strany 17–32    zošit 3: strany 33–40
```

V náhľade je každý zošit oddelený nadpisom a pri každom liste je uvedené,
koľký list zošita to je — podľa toho sa listy skladajú do seba.

### 2 strany na list – bez skladania

Poradie sa nemení (1|2, 3|4, …). Na šetrenie papiera pri čítaní, nie na väzbu.

## Ďalšie nastavenia

| Nastavenie | Načo je |
|---|---|
| **Formát / orientácia** | A2–A6, Letter, Legal, Tabloid, alebo *podľa zdroja* (list presne dvojnásobok strany, nič sa nezmenšuje) |
| **Väzba vľavo / vpravo** | vpravo pre jazyky písané sprava doľava a mangu |
| **Strany** | rozsah, napr. `1-8,11`; opačný rozsah (`8-1`) obráti poradie |
| **Okraj** | prázdny okraj po celom obvode listu |
| **Prehyb** | extra medzera v mieste prehybu (na väzbu / diery) |
| **Prehyb značiť** | krátke značky pri hranách listu (nekreslia sa cez obsah — pre hotové knižky), prerušovaná čiara cez celý list, alebo nič |
| **Orezové značky** | značky na hranách listu v mieste hrán strán |
| **Obrat papiera** | musí sedieť s duplexom v ovládači tlačiarne — ak vyjde rub hlavou dolu, prepni to |
| **Poradie** | prekladane (duplexná tlačiareň) alebo najprv líca a potom ruby (ručný duplex) |
| **Creep** | posunie obsah vonkajších listov k prehybu, aby po orezaní vyšli okraje rovnako |
| **Prispôsobiť mierku** | vypni, ak chceš mierku 1:1 |
| **Náhľady strán** | vykreslí v náhľade skutočný obsah strán; číslo strany sa presunie do rohového odznaku |

Miesto prehybu sa predvolene vyznačí krátkymi značkami pri hornej a dolnej
hrane listu — vidno, kde prehnúť, a nič sa nekreslí cez obsah strán. Ak
potrebuješ výraznejšie vodidlo, prepni na prerušovanú čiaru cez celý list;
tá však v hotovej knižke zostane vytlačená.

Ak počet strán nie je násobkom 4, doplnia sa prázdne miesta na konci — teda na
zadnú obálku, nikdy nie pred prvú stranu.

## Ako to vytlačiť

1. Otvor výstupné PDF v prezerači a tlač **bez akéhokoľvek ďalšieho
   zmenšovania** („actual size“ / „100 %“, nie „fit to page“).
2. Zapni obojstrannú tlač. Pri liste na ležato zvyčajne funguje **obrat po
   krátkej hrane**; ak sú ruby hlavou dolu, prepni v aplikácii voľbu
   *Obrat papiera*.
3. Bez duplexnej tlačiarne zvoľ *Ručný duplex – najprv líca*, vytlač líca,
   vlož stoh naspäť a vytlač ruby. Ak tlačiareň vracia stoh obrátený, použi
   variant *ruby odzadu*.
4. Listy nasklad na seba (nie po jednom skladaj!), prehni v strede a zošij
   alebo zosponkuj v prehybe.

Pred plnou tlačou sa vyplatí skúsiť to na 4 stranách.

## Preklad a spustenie

Treba Rust 1.92+ a vývojové balíky GTK 4.

```bash
# Debian / Ubuntu
sudo apt install libgtk-4-dev build-essential
# Fedora
sudo dnf install gtk4-devel gcc
# macOS
brew install gtk4 pkg-config
# Windows: gvsbuild alebo MSYS2 (mingw-w64-x86_64-gtk4)

cargo build --release
./target/release/booklet-gui           # grafická aplikácia
./target/release/booklet-gui kniha.pdf # alebo hneď so súborom
```

Knižnica `booklet-core` na GTK nezávisí, takže `cargo build -p booklet-core`
a `cargo test` fungujú aj bez nainštalovaného GTK.

## Príkazová riadka

```bash
# brožúra na A4 na ležato
booklet kniha.pdf -o kniha-tlac.pdf

# zošity po 4 listoch (16 strán), 8 mm na väzbu, orezové značky
booklet kniha.pdf --mode signatures -n 4 --gutter 8 --crop

# bez akýchkoľvek značiek
booklet kniha.pdf --fold none

# len si pozri, čo kde skončí
booklet kniha.pdf --mode signatures -n 2 --dry-run
```

`booklet --help` vypíše všetky voľby.

## Testy

```bash
cargo test
```

`booklet-core` má jednotkové testy na poradie strán a geometriu a end-to-end
testy, ktoré vygenerujú PDF, prepočítajú ho a späť overia, ktorá zdrojová
strana skončila v ktorom slote. `booklet-gui` má smoke testy kreslenia
náhľadu (vrátane otočených slotov a všetkých variantov značiek).
