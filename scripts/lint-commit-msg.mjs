#!/usr/bin/env node
// lint-commit-msg.mjs — refuse un message de commit dont une ligne française est écrite sans le
// moindre accent alors qu'elle contient un mot dont l'accent est OBLIGATOIRE.
//
// POURQUOI CE FICHIER EXISTE. La cause du strip d'accents a été mesurée le 2026-08-26 (issue #43) :
// ce n'est ni une session, ni un mode, ni un bug d'encodage, mais le texte destiné à un ARGUMENT DE
// LIGNE DE COMMANDE, rédigé en ASCII par prudence de quoting. Le discriminant est venu de `gh`, où
// le titre d'une issue passe par `--title` et son corps par `--body-file` : sur 47 issues à corps
// substantiel, ZÉRO corps désaccentué, alors que des titres l'étaient. Le canal fichier n'a jamais
// strippé ; le canal argument, si. Côté git, `commit -m "titre" -m "corps"` met les deux en
// arguments et `commit -F fichier` les met dans un fichier — d'où 43 messages tombés d'un bloc
// contre 147 intacts, et cinq journées mixtes.
//
// `lint-accents.mjs` garde les fichiers du dépôt. Le message de commit, lui, n'est dans aucun
// fichier suivi : c'est le seul canal qui restait sans gate, et c'est justement le plus touché.
//
// PAS EN CI, et c'est un choix : une étape CI échouerait sur un message DÉJÀ POUSSÉ, donc
// irréparable sans réécriture d'historique. Un hook `commit-msg` refuse avant que le commit existe.
//
// Usage : node scripts/lint-commit-msg.mjs <fichier-message>   (c'est le contrat d'un hook commit-msg)

import { readFileSync } from 'node:fs';

// Formes NUES dont la version accentuée est obligatoire en français, ET qui ne sont ni un mot
// anglais ni un identifiant de code plausible. Les homographes (`execute`, `cache`, `declare`,
// `charge`, `note`, `role`, `element`, `reference`, `selection`, `mode`, `type`) sont exclus à
// dessein : mesurés le 2026-08-26, ils produisaient à eux seuls 681 des 1932 faux positifs du scan
// par mots sur le dépôt.
const FAUTIFS =
  /\b(deja|regle|regles|echec|echecs|echoue|echouee|defaut|defauts|fenetre|fenetres|depot|echelle|systeme|systemes|litteral|duree|durees|ecart|ecarts|tres|apres|plutot|cout|couts|probleme|problemes|premiere|derniere|entiere|reponse|precedent|precedente|necessaire|numero|perime|perimee|interet|controle|desormais|reel|reelle|resultat|resultats|separateur|verifie|verifiee|verifier|verification|verifications|verite|securite|priorite|identite|densite|opacite|reussi|arrete|arretee|carre|carree|modele|modeles|critere|criteres|parametre|parametres|caractere|caracteres|requete|requetes|maniere|memoire|prealable|unite|unites|qualite|integrite|epingle|epinglee|delibere|deliberee|developpement|integration|definition|execution|telecharge|telechargee|publie|publiee|separe|separee|independant|sequentiel|recopiee|negatif|desactiver|hierarchie|dedoublonnage|enumere|declarations|chargee|ecrit|ecrite|creee|generique|numerote|amelioration|derive|derivee|mesuree|documentee|repere|detecte|declare|retiree|livree|corrigee|supprimee|affichee|appelee|poussee|tranchee)\b/i;
// Contexte français : sans lui, un sujet anglais contenant par hasard une de ces formes déclencherait.
const OUTILS =
  /\b(le|la|les|une|des|du|qui|que|dont|pour|dans|sous|avec|sans|pas|ne|est|sont|cette|ces|leur|elle|ils|donc|mais|car|tous|toute|meme|deja|encore|jamais|quand|comme|alors|ainsi|puis|entre|vers|apres|avant|depuis|selon|faut|fait|faire|etre|avoir|peut|doit|aucun|aucune|chaque|plutot|parce|lorsque|afin|rien|tout|au|aux|sur|par|ce|il|un)\b/gi;
const ACCENTS = /[éèêëàâäùûüîïôöçœÉÈÊËÀÂÄÙÛÜÎÏÔÖÇŒ]/;

const file = process.argv[2];
if (!file) {
  console.error('usage: node scripts/lint-commit-msg.mjs <fichier-message>');
  process.exit(2);
}

const brut = readFileSync(file, 'utf8');
// Les lignes de commentaire de git (`#`) ne font pas partie du message, et `git commit -v` y colle
// le diff entier — le lire ferait échouer sur le code d'autrui.
const lignes = brut.split(/\r?\n/).filter((l) => !l.startsWith('#'));

/** Le texte de la ligne, moins ce qui n'est PAS de la prose française.
 *
 *  Deux retraits, tous deux mesurés sur un faux positif réel (le commit `70f3340`, qui décrit
 *  justement ce bug et se faisait refuser en le citant) :
 *
 *  - le **code inline** entre backticks — `echelle` est un nom de variable Rust, `perime.flac` une
 *    fixture, `data-view="ecarts"` un identifiant de vue. Aucun n'a d'accent à porter, et les
 *    accentuer casserait le code ;
 *  - une **citation d'un ou deux mots** entre guillemets français — « verification » désigne le mot
 *    lui-même, pas son sens. Deux mots au plus : au-delà, c'est une citation de prose, qui doit
 *    rester soumise à la règle. */
function prose(l) {
  return l.replace(/`[^`]*`/g, ' ').replace(/«\s*\S+(?:\s+\S+)?\s*»/g, ' ');
}

const fautives = [];
lignes.forEach((l, i) => {
  if (ACCENTS.test(l)) return;
  const texte = prose(l);
  const mot = texte.match(FAUTIFS);
  if (!mot) return;
  const outils = new Set((texte.match(OUTILS) || []).map((w) => w.toLowerCase()));
  if (outils.size >= 2) fautives.push({ n: i + 1, mot: mot[0], l: l.trim() });
});

if (!fautives.length) process.exit(0);

console.error('lint-commit-msg: ÉCHEC — ce message contient du français désaccentué.\n');
for (const f of fautives) console.error(`  ligne ${f.n} · « ${f.mot} » : ${f.l.slice(0, 110)}`);
console.error(
  "\nCause mesurée (issue #43) : le texte destiné à un ARGUMENT de ligne de commande est rédigé en\n" +
    "ASCII par prudence de quoting, et l'évitement déborde sur la prose. Le remède est d'éliminer le\n" +
    'canal, pas de retaper le message :\n\n' +
    "  git commit -F -  <<'EOF'\n  titre accentué\n\n  corps accentué\n  EOF\n\n" +
    'Le canal fichier n\'a jamais strippé sur ce dépôt — 47 corps d\'issue sur 47, aucun touché.',
);
process.exit(1);
