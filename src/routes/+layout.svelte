<script>
  import { browser } from "$app/environment";
  import { onMount } from "svelte";
  import { initAppListeners } from "$lib/listeners";
  import "../app.css";

  let { children } = $props();

  onMount(initAppListeners);

  if (browser) {
    document.addEventListener("contextmenu", (e) => {
      e.preventDefault();
    });
  }
</script>

<div
  class="w-screen h-screen max-w-[100vw] max-h-screen *:size-full font-custom"
>
  {@render children?.()}
</div>
