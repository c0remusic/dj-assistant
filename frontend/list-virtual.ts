// Shared list virtualization for the views whose scroll container holds MORE than the list
// (Bibliothèque, Écartés). Generalizes the #ql windowing pattern already proven in sift-live.ts
// (renderQueueWindow/measureQueueRowHeight, memory: sift-large-queue-black-screen): only the rows
// inside the visible scroll window (+ a buffer above/below) are mounted, framed by two spacer divs
// so the scrollbar stays proportional to the full list.
//
// Difference from #ql: there the whole scroll container IS the list, so scrollTop maps directly to
// row index. Here the scroll container is #content (set to overflow-y:auto by app.js's block()),
// but the list is only ONE section of it — stats/rekordbox/header/facets sit ABOVE it. So the
// window is computed from `scrollContainer.scrollTop` MINUS the list host's offsetTop within that
// container, and the list host is given `position:relative` so the spacers reserve exactly the
// full-list height without disturbing the surrounding layout.
//
// The state (selection Sets, counts, totals) always lives in the caller's DATA — this module only
// mounts a window of the data as DOM. A row outside the window simply isn't in the DOM; its data
// (and any selection membership) is untouched. Never derive counts/selection from the mounted DOM.

const ROW_BUFFER = 12; // rows mounted above/below the visible window (matches #ql's discipline)

export interface VirtualList {
  /** Re-mount the visible window from the current data. Call after the data changes. */
  render(): void;
  /** Detach the scroll listener (call before discarding the host, e.g. a full view re-render). */
  destroy(): void;
}

/**
 * Windows `items` into `host`, reading scroll position from `scrollContainer`.
 *
 * @param host           The element that will contain the mounted rows + spacers. Must be a child
 *                       (any depth) of scrollContainer. Given position:relative here.
 * @param scrollContainer The element that actually scrolls (its scrollTop drives the window).
 * @param items          Full data array (the source of truth — never mutated here).
 * @param rowHtml        Builds one row's HTML from an item. Every row MUST render the same height
 *                       (the whole scheme relies on a single measured row height); callers that
 *                       have variable-height rows must pad to a fixed height instead.
 * @param probeHtml      Markup for an offscreen probe used to measure one row's real height once
 *                       (never assumed — same discipline as measureQueueRowHeight).
 * @param fallbackRowH   Height (px) to use if the probe measures 0 (measured off-DOM) — never 0.
 */
export function createVirtualList<T>(opts: {
  host: HTMLElement;
  scrollContainer: HTMLElement;
  items: readonly T[];
  rowHtml: (item: T, index: number) => string;
  probeHtml: string;
  fallbackRowH: number;
}): VirtualList {
  const { host, scrollContainer, items, rowHtml, probeHtml, fallbackRowH } = opts;
  // Spacers reserve the off-window height; position:relative so offsetTop math is stable and the
  // host occupies exactly the full-list height in the surrounding flow.
  host.style.position = "relative";

  let rowH: number | null = null;
  function measureRowH(): number {
    if (rowH != null) return rowH;
    const probe = document.createElement("div");
    probe.style.position = "absolute";
    probe.style.visibility = "hidden";
    probe.style.pointerEvents = "none";
    probe.style.left = "0";
    probe.style.right = "0";
    probe.innerHTML = probeHtml;
    host.appendChild(probe);
    const h = probe.getBoundingClientRect().height;
    probe.remove();
    rowH = h > 0 ? h : fallbackRowH;
    return rowH;
  }

  function render(): void {
    if (!items.length) {
      host.innerHTML = "";
      return;
    }
    const h = measureRowH();
    // The list host's top relative to the scroll container's own scroll origin. offsetTop is
    // relative to offsetParent; walk up to the scroll container to get the true offset (the host
    // may be nested inside fl/facet wrappers). Read layout once per render (bounded by the
    // rAF-throttled scroll handler below, so not per fired scroll event).
    const hostTop = host.getBoundingClientRect().top - scrollContainer.getBoundingClientRect().top
      + scrollContainer.scrollTop;
    const viewportH = scrollContainer.clientHeight || 600;
    // scrollTop position expressed relative to the first row.
    const rel = scrollContainer.scrollTop - hostTop;
    const start = Math.max(0, Math.floor(rel / h) - ROW_BUFFER);
    const visibleCount = Math.ceil(viewportH / h) + ROW_BUFFER * 2;
    const end = Math.min(items.length, start + visibleCount);
    const topSpacer = start * h;
    const bottomSpacer = (items.length - end) * h;
    let html = topSpacer > 0 ? `<div style="height:${topSpacer}px"></div>` : "";
    for (let i = start; i < end; i++) html += rowHtml(items[i], i);
    if (bottomSpacer > 0) html += `<div style="height:${bottomSpacer}px"></div>`;
    host.innerHTML = html;
  }

  // Passive, rAF-throttled scroll listener. The scroll event fires at a high frequency (dozens/sec
  // during a flick); coalescing to one render per animation frame keeps the window fresh without
  // rebuilding the DOM on every fired event.
  let ticking = false;
  const onScroll = (): void => {
    if (ticking) return;
    ticking = true;
    requestAnimationFrame(() => {
      ticking = false;
      // Guard against a stale listener firing after the host was replaced by a full re-render.
      if (!host.isConnected) return;
      render();
    });
  };
  scrollContainer.addEventListener("scroll", onScroll, { passive: true });

  render();
  return {
    render,
    destroy(): void {
      scrollContainer.removeEventListener("scroll", onScroll);
    },
  };
}
