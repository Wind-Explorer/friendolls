<script lang="ts">
  import { onMount } from "svelte";
  import { startHitboxSync } from "./hitboxes";
  import { liveMetadata } from "$lib/listeners/live-metadata";

  onMount(startHitboxSync);
</script>

<div class="size-full flex flex-col justify-end">
  <div role="banner" class="relative h-8 w-full my-2">
    {#each Object.entries($liveMetadata.cursorPositions) as [userId, cursor] (userId)}
      {@const ico = $liveMetadata.foregroundApps.get(userId)?.ico}
      {#if cursor}
        <div
          class="scene-hitbox absolute bottom-0 left-0 flex flex-col gap-1 items-center transition-transform ease-linear duration-1000"
          style:transform={`translateX(${cursor.mapped.x * 100}vw)`}
        >
          {#if ico}
            <img
              src={`data:image/png;base64,${ico}`}
              alt=""
              class="size-4 object-contain"
            />
          {/if}
          <img
            src="/fa.png"
            alt=""
            class="size-6"
            style="filter: sepia(1) saturate(200%) hue-rotate(100deg);"
          />
        </div>
      {/if}
    {/each}
  </div>
</div>
