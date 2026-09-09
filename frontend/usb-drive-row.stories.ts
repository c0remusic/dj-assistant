import type { Meta, StoryObj } from "@storybook/html-vite";
import type { RemovableDrive } from "./ipc";
import { usbEntryHtml } from "./usb-row";

// Les trois états réels d'une entrée de la colonne des disques de l'écran Clé USB, rendus par la
// MÊME fonction que `paint()` (usb-view.ts) — pas une copie du markup, qui dériverait.
//
// Jusqu'au 2026-09-09 c'était une LIGNE de carte (lettre · modèle · taille · Formater…). Lu contre
// Utilitaire de disque (déclinaison #24, sixième écran), un disque est une entrée de sidebar :
// glyphe, nom, capacité à droite, au plan de la file — le modèle, le système de fichiers et les
// actions vivent en zone C. Les deux derniers états n'existaient pas avant le 2026-07-31 :
// l'énumération partait du volume logique, donc un disque sans volume monté ne pouvait pas
// apparaître du tout. C'est ce qui rendait l'écran inutilisable, une clé neuve ou RAW étant
// exactement ce qu'on vient formater.
type Args = RemovableDrive & { on: boolean };

const meta: Meta<Args> = {
  title: "États de contenu/Entrée disque amovible",
  render: (args) => {
    const side = document.createElement("nav");
    side.className = "sift-usb-side";
    side.style.width = "var(--pane-w)";
    side.innerHTML = '<div class="col-h">Disques amovibles</div>' + usbEntryHtml(args, args.on);
    return side;
  },
  argTypes: {
    label: { control: "text" },
    mount: { control: "text" },
    current_fs: { control: "text" },
    size_bytes: { control: "number" },
    has_media: { control: "boolean" },
    on: { control: "boolean" },
  },
};

export default meta;
type Story = StoryObj<Args>;

/** Cas nominal : clé déjà formatée et montée, choisie. L'identifiant affiché est la lettre. */
export const Formatee: Story = {
  args: {
    id: "\\\\.\\PHYSICALDRIVE2",
    label: "Kingston DataTraveler USB Device",
    mount: "E:",
    size_bytes: 16_000_000_000,
    free_bytes: 5_300_000_000,
    current_fs: "FAT32",
    volume_name: "USB DJ",
    health: "OK",
    has_media: true,
    identity: "USBSTOR\\X|SN123|16000000000|AAAA-1111",
    on: true,
  },
};

/** Clé neuve ou RAW : aucun volume monté, donc aucune lettre — l'identifiant retombe sur le
 * numéro de disque. Formatable, et c'est le point : cette entrée n'existait pas avant. */
export const NonFormatee: Story = {
  args: {
    id: "\\\\.\\PHYSICALDRIVE3",
    label: "SanDisk Ultra USB Device",
    mount: "",
    size_bytes: 32_000_000_000,
    free_bytes: 0,
    current_fs: "non formaté",
    volume_name: "",
    health: "",
    has_media: true,
    identity: "USBSTOR\\Y||32000000000|",
    on: false,
  },
};

/** Lecteur de cartes énuméré mais vide. Windows lui garde une lettre dans l'explorateur, d'où la
 * confusion « je vois bien un lecteur USB » — l'entrée dit « vide », et la zone C explique ;
 * `format_drive` refuserait de toute façon (`has_media: false`). */
export const SansMedia: Story = {
  args: {
    id: "\\\\.\\PHYSICALDRIVE4",
    label: "Generic SD Reader",
    mount: "F:",
    size_bytes: 0,
    free_bytes: 0,
    current_fs: "",
    volume_name: "",
    health: "",
    has_media: false,
    identity: "USBSTOR\\Z|||",
    on: false,
  },
};
