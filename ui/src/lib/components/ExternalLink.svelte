<script lang="ts">
  import type { Snippet } from "svelte";
  import { openExternal } from "../api/links";

  interface Props {
    href: string;
    children: Snippet;
  }

  let { href, children }: Props = $props();

  function onclick(e: MouseEvent) {
    e.preventDefault();
    void openExternal(href);
  }
</script>

<a {href} target="_blank" rel="noreferrer" {onclick}>{@render children()}</a>
