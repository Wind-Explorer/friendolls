<script lang="ts">
  import { onMount } from "svelte";
  import Popover from "$lib/components/popover.svelte";
  import { friends } from "$lib/listeners/friends";
  import { incomingInteraction } from "$lib/listeners/interactions";
  import { liveMetadata } from "$lib/listeners/live-metadata";
  import { profile } from "$lib/listeners/profile";
  import SceneImageViewer from "./popovers/image-viewer.svelte";
  import SceneInteractionBubble from "./popovers/interaction-bubble.svelte";
  import SceneUserPopoverContent from "./popovers/user-interaction.svelte";
  import { startHitboxSync } from "./hitboxes";

  let selectedUserId = $state<string | null>(null);
  let selectedUserX = $state<number | null>(null);
  let lockedPopoverUserId = $state<string | null>(null);
  let viewedImage = $state<{ source: string; senderName: string } | null>(null);

  onMount(startHitboxSync);

  $effect(() => {
    const current = $incomingInteraction;
    if (!current) return;
    const duration = current.content.type === "wave" ? 4_000 : 10_000;
    const timer = window.setTimeout(
      () => dismissInteraction(current.interactionId),
      duration,
    );
    return () => window.clearTimeout(timer);
  });

  function displayName(userId: string) {
    return $profile?.id === userId
      ? $profile.displayName
      : ($friends.find((friend) => friend.id === userId)?.displayName ??
          "Unknown user");
  }

  function dismissInteraction(interactionId: string) {
    incomingInteraction.update((current) =>
      current?.interactionId === interactionId ? null : current,
    );
  }

  function setPopoverOpen(
    userId: string,
    cursorX: number,
    open: boolean,
    trigger: HTMLButtonElement,
  ) {
    if (open && lockedPopoverUserId && lockedPopoverUserId !== userId) return;
    selectedUserId = open ? userId : null;
    if (!open && lockedPopoverUserId === userId) lockedPopoverUserId = null;
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
      {@const interaction =
        $incomingInteraction?.friendId === userId ? $incomingInteraction : null}
      {#if cursor}
        {@const renderedX =
          selectedUserId === userId ? selectedUserX : cursor.mapped.x}
        <div
          class="absolute bottom-0 left-0 flex flex-col items-center gap-1 transition-transform duration-1000 ease-linear"
          style:transform={`translateX(${(renderedX ?? cursor.mapped.x) * 100}vw)`}
        >
          {#if interaction}
            <SceneInteractionBubble
              {interaction}
              senderName={displayName(userId)}
              onDismiss={() => dismissInteraction(interaction.interactionId)}
              onOpenImage={(source) => {
                viewedImage = { source, senderName: displayName(userId) };
                dismissInteraction(interaction.interactionId);
              }}
            />
          {/if}

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
            closeOnOutsidePointer={lockedPopoverUserId !== userId}
            closeOnWindowBlur={lockedPopoverUserId !== userId}
            closeOnFocusOut={lockedPopoverUserId !== userId}
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
              {foregroundApp}
              onModeChange={(active) =>
                (lockedPopoverUserId = active ? userId : null)}
              onDismiss={() => {
                selectedUserId = null;
                selectedUserX = null;
                lockedPopoverUserId = null;
              }}
              onSent={() => {
                selectedUserId = null;
                selectedUserX = null;
                lockedPopoverUserId = null;
              }}
            />
          </Popover>
        </div>
      {/if}
    {/each}
  </div>
</div>

{#if viewedImage}
  <SceneImageViewer
    source={viewedImage.source}
    senderName={viewedImage.senderName}
    onClose={() => (viewedImage = null)}
  />
{/if}
