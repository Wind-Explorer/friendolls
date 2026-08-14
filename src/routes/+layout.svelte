<script>
  import { browser } from "$app/environment";
  import "../app.css";

  let { children } = $props();
  if (browser) {
    document.addEventListener("contextmenu", (e) => {
      e.preventDefault();
    });
  }
</script>

<div class="w-screen h-screen max-w-[100vw] max-h-screen *:size-full">
  {@render children?.()}
</div>
