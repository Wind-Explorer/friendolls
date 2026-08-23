<script lang="ts">
  import { onMount } from "svelte";
  import Popover from "$lib/components/popover.svelte";
  import { liveMetadata } from "$lib/listeners/live-metadata";
  import SceneUserPopoverContent from "./scene-user-popover-content.svelte";
  import { startHitboxSync } from "./hitboxes";

  let selectedUserId: string | null = null;
  let selectedUserX: number | null = null;

  onMount(startHitboxSync);

  function setPopoverOpen(
    userId: string,
    cursorX: number,
    open: boolean,
    trigger: HTMLButtonElement,
  ) {
    selectedUserId = open ? userId : null;
    selectedUserX = open
      ? window.innerWidth > 0
        ? trigger.getBoundingClientRect().left / window.innerWidth
        : cursorX
      : null;
  }
</script>

<div class="flex size-full flex-col justify-end">
  <div role="banner" class="relative my-2 h-8 w-full">
    {#each Object.entries($liveMetadata.cursorPositions) as [userId, cursor], index (userId)}
      {@const foregroundApp = $liveMetadata.foregroundApps.get(userId)}
      {@const popoverId = `scene-user-${index}`}
      {#if cursor}
        {@const renderedX = selectedUserId === userId ? selectedUserX : cursor.mapped.x}
        <div
          class="absolute bottom-0 left-0 flex flex-col items-center gap-1 transition-transform duration-1000 ease-linear"
          style:transform={`translateX(${(renderedX ?? cursor.mapped.x) * 100}vw)`}
        >
          {#if foregroundApp?.ico}
            <img
              src={`data:image/png;base64,${foregroundApp.ico}`}
              alt=""
              class="size-4 object-contain"
            />
          {/if}

          <Popover
            id={popoverId}
            label="Show live user information"
            labelledBy={`${popoverId}-title`}
            open={selectedUserId === userId}
            onOpenChange={(open, trigger) =>
              setPopoverOpen(userId, cursor.mapped.x, open, trigger)}
            triggerClass="scene-hitbox"
            panelClass="scene-hitbox bottom-[calc(100%+0.75rem)] w-68"
            panelStyle={`left: clamp(calc(0.5rem - ${(renderedX ?? cursor.mapped.x) * 100}vw), -8rem, calc(100vw - ${(renderedX ?? cursor.mapped.x) * 100}vw - 17.5rem))`}
          >
            {#snippet trigger()}
              <img
                src="/fa.png"
                alt=""
                class="size-6 sepia saturate-200 hue-rotate-[100deg]"
              />
            {/snippet}

            <SceneUserPopoverContent
              titleId={`${popoverId}-title`}
              {userId}
              isLocal={userId === $liveMetadata.localId}
              {cursor}
              {foregroundApp}
            />
          </Popover>
        </div>
      {/if}
    {/each}
  </div>
</div>
