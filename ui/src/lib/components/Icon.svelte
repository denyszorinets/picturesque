<script lang="ts" module>
  // Heroicons (MIT), the 20 px solid set, bundled so the app works offline.
  // Add an icon by importing its SVG here.
  import archiveBox from "heroicons/20/solid/archive-box.svg?raw";
  import arrowPath from "heroicons/20/solid/arrow-path.svg?raw";
  import arrowUpTray from "heroicons/20/solid/arrow-up-tray.svg?raw";
  import arrowUturnLeft from "heroicons/20/solid/arrow-uturn-left.svg?raw";
  import arrowUturnRight from "heroicons/20/solid/arrow-uturn-right.svg?raw";
  import chartBar from "heroicons/20/solid/chart-bar.svg?raw";
  import check from "heroicons/20/solid/check.svg?raw";
  import checkCircle from "heroicons/20/solid/check-circle.svg?raw";
  import chevronDown from "heroicons/20/solid/chevron-down.svg?raw";
  import chevronLeft from "heroicons/20/solid/chevron-left.svg?raw";
  import chevronRight from "heroicons/20/solid/chevron-right.svg?raw";
  import cog6Tooth from "heroicons/20/solid/cog-6-tooth.svg?raw";
  import documentDuplicate from "heroicons/20/solid/document-duplicate.svg?raw";
  import exclamationTriangle from "heroicons/20/solid/exclamation-triangle.svg?raw";
  import folder from "heroicons/20/solid/folder.svg?raw";
  import folderOpen from "heroicons/20/solid/folder-open.svg?raw";
  import funnel from "heroicons/20/solid/funnel.svg?raw";
  import informationCircle from "heroicons/20/solid/information-circle.svg?raw";
  import lightBulb from "heroicons/20/solid/light-bulb.svg?raw";
  import magnifyingGlass from "heroicons/20/solid/magnifying-glass.svg?raw";
  import magnifyingGlassPlus from "heroicons/20/solid/magnifying-glass-plus.svg?raw";
  import photo from "heroicons/20/solid/photo.svg?raw";
  import questionMarkCircle from "heroicons/20/solid/question-mark-circle.svg?raw";
  import rectangleStack from "heroicons/20/solid/rectangle-stack.svg?raw";
  import sparkles from "heroicons/20/solid/sparkles.svg?raw";
  import squares2x2 from "heroicons/20/solid/squares-2x2.svg?raw";
  import star from "heroicons/20/solid/star.svg?raw";
  import trash from "heroicons/20/solid/trash.svg?raw";
  import xCircle from "heroicons/20/solid/x-circle.svg?raw";
  import xMark from "heroicons/20/solid/x-mark.svg?raw";

  const ICONS = {
    "archive-box": archiveBox,
    "arrow-path": arrowPath,
    "arrow-up-tray": arrowUpTray,
    "arrow-uturn-left": arrowUturnLeft,
    "arrow-uturn-right": arrowUturnRight,
    "chart-bar": chartBar,
    "light-bulb": lightBulb,
    check: check,
    "check-circle": checkCircle,
    "chevron-down": chevronDown,
    "chevron-left": chevronLeft,
    "chevron-right": chevronRight,
    "cog-6-tooth": cog6Tooth,
    "document-duplicate": documentDuplicate,
    "exclamation-triangle": exclamationTriangle,
    folder: folder,
    "folder-open": folderOpen,
    funnel: funnel,
    "information-circle": informationCircle,
    "magnifying-glass": magnifyingGlass,
    "magnifying-glass-plus": magnifyingGlassPlus,
    photo: photo,
    "question-mark-circle": questionMarkCircle,
    "rectangle-stack": rectangleStack,
    sparkles: sparkles,
    "squares-2x2": squares2x2,
    star: star,
    trash: trash,
    "x-circle": xCircle,
    "x-mark": xMark,
  } as const;

  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  interface Props {
    name: IconName;
    /** Read out instead of the text next to it; without one the icon is
     * decorative and hidden from screen readers. */
    label?: string;
  }

  let { name, label }: Props = $props();
  const svg = $derived(ICONS[name]);
</script>

<span
  class="icon"
  role={label ? "img" : undefined}
  aria-label={label}
  aria-hidden={label ? undefined : "true"}
>
  <!-- Bundled SVG files, not user content. -->
  <!-- eslint-disable-next-line svelte/no-at-html-tags -->
  {@html svg}
</span>

<style>
  /* Sized by the surrounding text; set --icon-size to change it. */
  .icon {
    display: inline-flex;
    flex-shrink: 0;
    width: var(--icon-size, 1.15em);
    height: var(--icon-size, 1.15em);
    vertical-align: -0.2em;
  }

  .icon :global(svg) {
    width: 100%;
    height: 100%;
  }
</style>
