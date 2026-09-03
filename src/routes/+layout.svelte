<script lang="ts">
  import { browser } from "$app/environment";
  import { onMount } from "svelte";
  import { initLocalization, locale } from "$lib/i18n";
  import { initAppListeners } from "$lib/listeners";
  import "../app.css";

  let { children } = $props();
  let ready = $state(false);

  onMount(() => {
    let disposeLocalization: (() => void) | undefined;
    let disposeListeners: (() => void) | undefined;
    let disposed = false;

    void initLocalization()
      .then((dispose) => {
        if (disposed) {
          dispose();
          return;
        }
        disposeLocalization = dispose;
        disposeListeners = initAppListeners();
        ready = true;
      })
      .catch((error) => {
        console.error("failed to initialize localization", error);
        if (!disposed) {
          disposeListeners = initAppListeners();
          ready = true;
        }
      });

    return () => {
      disposed = true;
      disposeListeners?.();
      disposeLocalization?.();
    };
  });

  if (browser) {
    document.addEventListener("contextmenu", (e) => {
      e.preventDefault();
    });
  }
</script>

{#if ready}
  <div
    class="w-screen h-screen max-w-[100vw] max-h-screen *:size-full"
    class:font-en={$locale !== "zh-CN"}
    class:font-zh-cn={$locale === "zh-CN"}
  >
    {@render children?.()}
  </div>
{/if}
