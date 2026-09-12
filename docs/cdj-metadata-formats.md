# Formats de métadonnées lus par les platines Pioneer / AlphaTheta

> Établi le 2026-08-21 par recherche sourcée (issue #46). **Ce document est la source de la
> matrice appliquée par le code.** Le critère `tags_cdj_ok` a été recâblé dessus le 2026-09-01 :
> `analysis/tags.rs::tag_type_readable_on_cdj` porte la table de vérité couple par couple, et
> `cdj_readable_matrix_matches_the_doc` (même fichier) la fige contre la matrice ci-dessous. Un
> changement ici se répercute là, dans le même geste. Avant cette date le code testait la seule
> présence d'Artiste+Titre, quel que soit le format du tag.
>
> Le badge « CDJ compatible » de Revue, lui, a été retiré de l'UI le 2026-08-25 : le champ existe
> et est juste, aucune surface ne l'affiche à ce jour.
>
> Périmètre : affichage d'Artiste / Titre en **navigation directe sur clé USB, sans base
> rekordbox**. Le cas « exporté via rekordbox » est différent (l'affichage vient alors de la
> base, pas du fichier) et explique une partie des rapports contradictoires.
>
> ⚠️ **Limite de collecte, honnête** : toutes les propriétés web officielles Pioneer /
> AlphaTheta (`support.alphatheta.com`, `forums.pioneerdj.com`, `community.pioneerdj.com`)
> ont renvoyé **HTTP 403** en fetch automatisé. Le contenu « primaire » ci-dessous vient du
> **texte des manuels officiels via le miroir `manualslib.com`** et des **snippets de
> résultats de recherche**. Les pages Pioneer n'ont pas été lues directement sur leur domaine.

## Le fond, en deux points

**1. Le conteneur WAV est le vrai cas problématique — pas l'ID3v2.4.** Les tags d'un WAV
(RIFF INFO `INAM`/`IART`, ou ID3 dans un chunk `id3 ` non standard) ne sont **pas affichés
de façon fiable** par la platine en navigation directe : elle retombe sur le **nom de
fichier**. C'est exactement là que le badge actuel ment — `lofty` mappe `IART`→`TrackArtist`,
donc un WAV taggé passe `tags_cdj_ok = true` alors que la platine affichera probablement le
nom de fichier.

**2. La version ID3 n'est PAS une contrainte documentée.** Les manuels Pioneer déclarent
**uniformément** le support d'ID3 v1, v1.1, v2.2.0, v2.3.0 **et v2.4.0** sur toute la gamme
(2009→2020). « La CDJ ne lit pas le v2.4 » est une croyance communautaire ; les incidents
réels ont pour cause **probable l'encodage du texte** (UTF-8 / UTF-16 vs Latin-1), pas le
numéro de version. À traiter comme prudence, pas comme un fait Pioneer.

Les différences **par génération** portent sur les **codecs** (FLAC / ALAC seulement à partir
de 2016), **pas** sur les formats de tags — aucune différence de lecture de tags par
génération n'est documentée.

## Matrice de synthèse

Lecture = « la platine affiche Artiste / Titre en navigation directe USB, sans base rekordbox ».

| Fichier | Type de tag (nom `lofty`) | Affiché en direct USB | Confiance | Rang source |
|---|---|---|---|---|
| MP3 | ID3v2.2 / v2.3 (`Id3v2`) | **OUI** | Haute | Primaire + communautaire |
| MP3 | ID3v2.4 (`Id3v2`) | OUI officiellement ; incidents réels (cause probable = encodage) | Moyenne | Primaire dit oui / comm. prudence |
| MP3 | ID3v1 / v1.1 (`Id3v1`) | OUI, champs courts (30 car., pas d'Unicode) | Moyenne-haute | Primaire |
| AIFF | ID3 dans chunk (`Id3v2`) | **OUI** | Moyenne-haute | Primaire + communautaire |
| AIFF | chunks natifs `NAME`/`AUTH` (`AiffText`) | **Non établi** | — | — |
| **WAV** | RIFF INFO `INAM`/`IART` (`RiffInfo`) | **NON fiable — souvent nom de fichier seul** | Basse (verdict : non fiable) | Communautaire fort |
| WAV | ID3 dans chunk `id3 ` (`Id3v2`) | Non établi (présumé non fiable) | — | Communautaire |
| FLAC | Vorbis comments (`VorbisComments`) | OUI (matériel FLAC-capable, 2016+) | Moyenne-haute | Primaire + communautaire |
| ALAC (.m4a) | MP4 ilst (`Mp4Ilst`) | OUI (matériel ALAC-capable, 2016+) | Moyenne | Primaire |
| AAC (.m4a/.aac) | MP4 ilst (`Mp4Ilst`) | OUI | Moyenne-haute | Primaire |

Texte primaire décisif — manuel CDJ-2000NXS2, « Playable Music File Formats » : « *The tag
information types which can be registered from a music file are ID3 tags (v1, v1.1, v2.2.0,
v2.3.0, and v2.4.0) or meta tags.* » Limite pochette : « *Files larger than 800 x 800 dots
cannot be displayed.* »

### Contrainte codec par génération (résultat identique du point de vue du badge : rien ne s'affiche si le fichier ne joue pas)

| Modèle | Année | WAV/AIFF | FLAC | ALAC |
|---|---|---|---|---|
| CDJ-2000 / 900 / NXS, XDJ-1000, XDJ-RX/RX2/RR | 2009-2018 | 48 kHz/24-bit | **N/A** | **N/A** |
| CDJ-2000NXS2 / TOUR1 | 2016 | 96 kHz/24-bit | oui | oui |
| CDJ-3000, OPUS-QUAD | 2020-2023 | 96 kHz/24-bit | oui | oui |
| XDJ-1000MK2 | 2016 | 48 kHz/24-bit | oui | oui |
| XDJ-RX3 / XZ | 2019-2021 | 48 kHz/24-bit | oui | N/A (ALAC) |

Un FLAC / ALAC taggé impeccablement reste **injouable** (donc non affiché) sur les modèles
d'avant 2016. Problème de codec, pas de tag — mais résultat identique pour l'utilisateur.

## Le cas WAV, à part

**Ne jamais considérer un WAV « CDJ-OK » sur la seule présence de tags dans le fichier.** Deux
problèmes s'empilent :

- **(a) Affichage — non fiable.** WAV n'a pas de format de tag standardisé. rekordbox lit /
  écrit le RIFF INFO (`INAM`=titre, `IART`=artiste) ; d'autres outils insèrent un ID3 dans un
  chunk `id3 ` non standard. Le consensus communautaire, convergent, est que la platine
  **n'affiche pas ces tags de façon fiable** en direct et retombe sur le **nom de fichier**.
  Nuance : après **export rekordbox**, le WAV s'affiche (depuis la base, pas depuis le
  fichier) — d'où les rapports contradictoires.
- **(b) Lecture — le piège `WAVE_FORMAT_EXTENSIBLE`.** Séparé de l'affichage : beaucoup de WAV
  hi-res (24-bit, téléchargements Bandcamp) portent le format `WAVE_FORMAT_EXTENSIBLE` (octets
  20-21 du header = `FE FF` au lieu de `01 00`) que les **anciennes** platines **refusent** en
  « unsupported file type ». Si Sift encode / écrit des WAV : produire du PCM standard
  (`WAVE_FORMAT_PCM`), jamais de l'EXTENSIBLE.

## Ce qui est implémentable côté Sift (`lofty`)

`TagType` de `lofty` : `Ape, Id3v1, Id3v2, Mp4Ilst, VorbisComments, RiffInfo, AiffText`. Chaque
`Tag` expose `tag.tag_type()`.

| `TagType` | Fichiers | Verdict badge | Note |
|---|---|---|---|
| `Id3v2` | MP3, AIFF, WAV (chunk `id3 `) | **OK — sauf si le fichier est un WAV** | voir gate conteneur |
| `Mp4Ilst` | AAC, ALAC | **OK** | ALAC injouable avant 2016 (codec) |
| `VorbisComments` | FLAC | **OK sur matériel FLAC-capable** | injouable avant 2016 |
| `Id3v1` | MP3 | OK faible | 30 car., pas d'Unicode |
| `RiffInfo` | WAV | **PAS OK** (non fiable) | cœur du correctif |
| `AiffText` | AIFF natif | **PAS OK par prudence** (non établi) | fail-fast |
| `Ape` | APE / WavPack | N/A | non jouable sur CDJ |

**Correctif appliqué le 2026-09-01** dans `analysis/tags.rs::tag_type_readable_on_cdj` — ce qui
était proposé ici, et ce qu'il en reste :

1. **Gater sur le conteneur : FAIT.** Le prédicat prend le couple `(file_type, tag_type)` et non le
   seul type de tag, et `file_type` vient de `tagged.file_type()` — donc des octets, jamais de
   l'extension. Le WAV ne compte sous aucun porteur, `RiffInfo` comme ID3-en-chunk. Le défaut de la
   table est `false` : tout couple que ce document n'établit pas ne compte pas.
2. Contrainte codec / génération (FLAC, ALAC = 2016+) : **hors périmètre du prédicat**, qui ne juge
   que le porteur — son doc-comment le dit. Pas de décision prise ici.
3. PCM standard à l'encodage WAV : **non traité**, ça appartient à `encode.rs`, pas à ce critère.

**Piège d'API à connaître** : `TagType::Id3v2` **n'expose pas** la sous-version — `lofty`
upgrade tout en v2.4 en interne (« *This covers all ID3v2 versions since they all get upgraded
to ID3v2.4* »). Pour lire la version d'origine il faut le tag concret `Id3v2Tag` +
`original_version()`. **Mais** puisque Pioneer supporte v2.2/2.3/2.4, distinguer la sous-version
**n'est pas nécessaire** pour le badge. Le champ `id3_version` ne porte donc toujours aucune
sous-version — mais il n'est plus le stub `Some("ID3")` posé sur l'extension : depuis le
2026-09-01 il nomme le ou les TYPES réels du porteur (`tags.rs::tag_type_name`), triés et joints
par `+`. Le vrai risque v2.4 (encodage) n'est pas lisible depuis le type de tag.

## Sources

**Primaire — texte de manuel officiel (via miroir `manualslib`, fetch réussi) :**
- CDJ-2000NXS2, « Playable Music File Formats » p.7 — https://www.manualslib.com/manual/1196325/Pioneer-Cdj-2000nxs2.html?page=7
- CDJ-2000NXS p.7 — https://www.manualslib.com/manual/712075/Pioneer-Cdj-2000nxs.html?page=7
- CDJ-3000 — https://www.manualslib.com/manual/1909473/Pioneer-Dj-Cdj-3000.html

**Primaire — pages officielles Pioneer / AlphaTheta (contenu via snippets ; WebFetch = 403) :**
- AlphaTheta « Which file formats can I play? » — https://support.alphatheta.com/en-us/articles/4408217704857
- CDJ-2000 Owner's Manual p.8 — https://www.manualowl.com/m/Pioneer/CDJ-2000/Manual/150024?page=8
- CDJ-3000 Instruction Manual (PDF officiel) — https://www.bhphotovideo.com/lit_files/630695.pdf

**Compilation de specs officielles (secondaire, fiable) :**
- `joeselway/Pioneer-DJ-File-Formats` (matrice codec + note WAV_EXTENSIBLE) — https://github.com/joeselway/Pioneer-DJ-File-Formats

**Communautaire — comportement des tags / cas WAV / encodage :**
- Digital DJ Tips — https://www.digitaldjtips.com/dj-software-secrets/
- CDM — https://cdm.link/how-to-avoid-usb-and-rekordbox-djing-failures-a-complete-guide/
- DJ TechTools, « Please Label Your Tracks » — https://djtechtools.com/2015/08/21/producers-please-label-your-tracks-id3-tags-correctly/
- Elektronauts, WAV_EXTENSIBLE — https://www.elektronauts.com/t/pioneer-usb-decks-solved-an-issue-with-wavs-not-being-recognised/199489
- `7olstoy/pioneer-wav-fixer` — https://github.com/7olstoy/pioneer-wav-fixer
- mp3tag community, ID3v2.4 UTF-8 — https://community.mp3tag.de/t/confused-about-id3v2-4-tagging-and-in-artist-names/54864

**Référence technique :**
- Wikipedia, ID3 (encodages par version) — https://en.wikipedia.org/wiki/ID3
- libsndfile #740 (chunk `id3 ` non standard dans WAV) — https://github.com/libsndfile/libsndfile/issues/740
- `lofty` `TagType` — https://docs.rs/lofty/0.24.0/lofty/tag/enum.TagType.html
- `lofty` `Id3v2Tag::original_version` — https://docs.rs/lofty/0.24.0/lofty/id3/v2/struct.Id3v2Tag.html

## Zones d'incertitude — à tester sur vraie platine avant de graver un comportement fin

1. **WAV RIFF INFO en lecture directe** — la platine lit-elle `INAM`/`IART` d'un WAV **copié à
   la main** (sans base rekordbox), ou toujours le nom de fichier ? Consensus = nom de fichier,
   mais aucun test primaire isolé (direct vs export rekordbox rarement désambiguïsés).
2. **WAV avec chunk ID3 `id3 `** (et non RIFF INFO) — s'affiche-t-il mieux ? Non établi.
3. **AIFF chunks natifs (`AiffText`) vs ID3-dans-AIFF** — `lofty` les distingue ; le badge
   pourrait les traiter différemment une fois le test fait.
4. **ID3v2.4 : version vs encodage** — isoler la cause des incidents « v2.4 ne s'affiche pas »
   (même piste en v2.3/Latin-1, v2.4/UTF-8, v2.4/UTF-16 sur la même platine).
5. **Encodage accepté par le firmware** — Pioneer ne publie pas quels encodages
   (Latin-1 / UTF-8 / UTF-16) il décode proprement, ni la limite de caractères affichés.
