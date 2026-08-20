<script lang="ts">
  import { onMount } from "svelte";
  import { startHitboxSync } from "./hitboxes";

  onMount(startHitboxSync);
</script>

<div class="size-full flex flex-col justify-end">
  <div role="banner" class="scene-hitbox w-full h-min bg-white text-black">
    <p class="w-max">WYD</p>
  </div>
</div>
