import type { Meta, StoryObj } from "@storybook/html-vite";

// Documents the error/warning pattern catalogued in
// docs/design-system-states.md ("Pattern d'erreur/échec — .sift-*-error/-fail/-warn",
// 9 sites, 2026-07-19 audit). No shared component exists on purpose (see that
// doc's rationale) — every site repeats the same structural convention:
// `<div class="sift-{site}-{error|fail|warn}">` + a `ti-alert-triangle` icon +
// message, using one of two token pairs:
//   warning = recoverable/pending  -> --color-text-warning / --color-background-warning
//   danger  = destructive/irreversible -> --color-text-danger / --color-background-danger
// This story renders that convention directly (not a real component — see
// frontend/styles.css classes .sift-tag-warn / .sift-usbfmt-error for the
// real call sites) so the two severities stay visually comparable and the
// "never a 3rd tint" rule is easy to spot-check.
interface ErrorPatternArgs {
  severity: "warning" | "danger";
  message: string;
}

function renderErrorPattern({ severity, message }: ErrorPatternArgs): HTMLElement {
  const el = document.createElement("div");
  const cls = severity === "danger" ? "sift-usbfmt-error" : "sift-tag-warn";
  el.className = cls;
  el.innerHTML = `<i class="ti ti-alert-triangle" aria-hidden="true"></i> ${message}`;
  return el;
}

const meta: Meta<ErrorPatternArgs> = {
  title: "États de contenu/Pattern erreur-avertissement",
  render: renderErrorPattern,
  argTypes: {
    severity: { control: "radio", options: ["warning", "danger"] },
    message: { control: "text" },
  },
};

export default meta;
type Story = StoryObj<ErrorPatternArgs>;

export const Warning: Story = {
  args: {
    severity: "warning",
    message: "Métadonnées non gravées sur le fichier — tags en attente.",
  },
};

export const Danger: Story = {
  args: {
    severity: "danger",
    message: "Formatage de la clé USB : action irréversible.",
  },
};
