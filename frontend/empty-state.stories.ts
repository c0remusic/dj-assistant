import type { Meta, StoryObj } from "@storybook/html-vite";
import { emptyStateHtml, wireEmptyState } from "./empty-state";

// Documents the shared empty-state component referenced in
// docs/design-system-states.md ("État vide" — DESIGN.md). Real markup, real
// classes from frontend/styles.css — same function the app itself calls
// (filing.ts / ecartes-view.ts / sift-live.ts).
const meta: Meta = {
  title: "États de contenu/Empty state",
  render: (args) => {
    const wrapper = document.createElement("div");
    wrapper.innerHTML = emptyStateHtml(args as Parameters<typeof emptyStateHtml>[0]);
    wireEmptyState(wrapper);
    return wrapper;
  },
  argTypes: {
    title: { control: "text" },
    note: { control: "text" },
    backToRevue: { control: "boolean" },
  },
};

export default meta;
type Story = StoryObj;

export const Base: Story = {
  args: {
    title: "Rien dans Écartés",
    note: "Les morceaux écartés pendant la Revue apparaîtront ici.",
    backToRevue: false,
  },
};

export const AvecLienRevue: Story = {
  args: {
    title: "Bibliothèque vide",
    note: "Ajoutez des sources dans Réglages pour commencer.",
    backToRevue: true,
  },
};
