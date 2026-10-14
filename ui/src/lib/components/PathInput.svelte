<script lang="ts">
  import { canPickFolders, pickFolders } from "../api";

  interface Props {
    /** Accessible label for the text field. */
    label: string;
    value?: string;
    placeholder?: string;
    multiple?: boolean;
    /** Called with folders chosen in the native dialog. */
    onpick?: (paths: string[]) => void;
    onsubmit?: (value: string) => void;
    testid?: string;
  }

  let {
    label,
    value = $bindable(""),
    placeholder = "/path/to/folder",
    multiple = false,
    onpick,
    onsubmit,
    testid,
  }: Props = $props();

  async function browse() {
    const paths = await pickFolders({ multiple, title: label });
    if (paths && paths.length > 0) {
      if (onpick) onpick(paths);
      else value = paths[0] ?? value;
    }
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "Enter" && onsubmit) {
      e.preventDefault();
      onsubmit(value);
    }
  }
</script>

<div class="path-input">
  <input
    type="text"
    aria-label={label}
    {placeholder}
    bind:value
    {onkeydown}
    spellcheck="false"
    autocomplete="off"
    data-testid={testid}
  />
  {#if canPickFolders()}
    <button type="button" onclick={browse}>Browse…</button>
  {/if}
</div>

<style>
  .path-input {
    display: flex;
    gap: 0.5rem;
  }

  .path-input input {
    flex: 1;
  }
</style>
