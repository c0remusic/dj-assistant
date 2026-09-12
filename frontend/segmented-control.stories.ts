import type { Meta, StoryObj } from "@storybook/html-vite";

// Documents the unified segmented control from docs/design-system-states.md
// (L603 "Pastille segmentée — .sift-seg/.sift-seg-opt, composant unique" —
// one component replacing 4 prior implementations, 6 call sites). No shared
// TS builder exists (each site rebuilds its own markup, see styles.css:1288
// comment), so this story reproduces the documented DOM shape directly:
// .sift-seg (track) > .sift-seg-opt.on (selected option).
interface SegmentedArgs {
  options: string[];
  selected: number;
}

function renderSegmented({ options, selected }: SegmentedArgs): HTMLElement {
  const track = document.createElement("div");
  track.className = "sift-seg";
  options.forEach((label, i) => {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = "sift-seg-opt" + (i === selected ? " on" : "");
    btn.textContent = label;
    btn.addEventListener("click", () => {
      track.querySelectorAll(".sift-seg-opt").forEach((el) => el.classList.remove("on"));
      btn.classList.add("on");
    });
    track.appendChild(btn);
  });
  return track;
}

const meta: Meta<SegmentedArgs> = {
  title: "Composants/Pastille segmentée",
  render: renderSegmented,
  argTypes: {
    selected: { control: "number" },
  },
};

export default meta;
type Story = StoryObj<SegmentedArgs>;

export const FormatUSB: Story = {
  args: { options: ["FAT32", "exFAT"], selected: 0 },
};

export const Apparence: Story = {
  args: { options: ["Clair", "Sombre", "Système"], selected: 2 },
};
