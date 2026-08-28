<script lang="ts">
  import { onMount } from "svelte";
  import Popover from "$lib/components/popover.svelte";
  import { friendName, friends } from "$lib/listeners/friends";
  import { onlineFriendIds } from "$lib/listeners/friend-statuses";
  import { incomingInteraction } from "$lib/listeners/interactions";
  import { liveMetadata } from "$lib/listeners/live-metadata";
  import { profile } from "$lib/listeners/profile";
  import { puppetStates } from "$lib/listeners/puppets";
  import Renderer from "./components/renderer.svelte";
  import type { PuppetScreenBounds } from "./components/renderer/types";
  import { startHitboxSync } from "./hitboxes";
  import SceneImageViewer from "./popovers/image-viewer.svelte";
  import SceneInteractionBubble from "./popovers/interaction-bubble.svelte";
  import SceneUserPopoverContent from "./popovers/user-interaction.svelte";

  let selectedUserId = $state<string | null>(null);
  let lockedPopoverUserId = $state<string | null>(null);
  let viewedImage = $state<{ source: string; senderName: string } | null>(null);
  let puppetBounds = $state<PuppetScreenBounds[]>([]);
  let puppetBoundsById = $derived(
    new Map(puppetBounds.map((bounds) => [bounds.id, bounds])),
  );
  let visiblePuppets = $derived(
    $puppetStates.filter(
      ({ id }) =>
        id === $liveMetadata.localId || $onlineFriendIds.has(id),
    ),
  );

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

  $effect(() => {
    if (
      selectedUserId &&
      !visiblePuppets.some((puppet) => puppet.id === selectedUserId)
    ) {
      dismissPopover();
    }
  });

  function displayName(userId: string) {
    return $profile?.id === userId
      ? $profile.displayName
      : friendName(
          $friends.find((friend) => friend.id === userId),
          "Unknown user",
        );
  }

  function dismissInteraction(interactionId: string) {
    incomingInteraction.update((current) =>
      current?.interactionId === interactionId ? null : current,
    );
  }

  function setPopoverOpen(userId: string, open: boolean) {
    if (open && lockedPopoverUserId && lockedPopoverUserId !== userId) return;
    if (open) {
      selectedUserId = userId;
    } else {
      dismissPopover();
    }
  }

  function dismissPopover() {
    selectedUserId = null;
    lockedPopoverUserId = null;
  }

  function popoverPosition(bounds: PuppetScreenBounds) {
    const centerX = bounds.x + bounds.width / 2;
    const horizontal = `position: fixed; left: clamp(0.5rem, calc(${centerX}px - 8.5rem), calc(100vw - 17.5rem));`;

    return bounds.y > window.innerHeight / 2
      ? `${horizontal} bottom: calc(100vh - ${bounds.y}px + 0.75rem);`
      : `${horizontal} top: ${bounds.y + bounds.height + 12}px;`;
  }
</script>

<div class="relative size-full">
  <Renderer
    puppets={visiblePuppets}
    frozenPuppetId={selectedUserId}
    onBoundsChange={(bounds) => (puppetBounds = bounds)}
  />

  <div role="banner" class="pointer-events-none fixed inset-0 z-10">
    {#each visiblePuppets as puppet, index (puppet.id)}
      {@const userId = puppet.id}
      {@const bounds = puppetBoundsById.get(userId)}
      {@const foregroundApp = $liveMetadata.foregroundApps.get(userId)}
      {@const popoverId = `scene-user-${index}`}
      {@const interaction =
        $incomingInteraction?.friendId === userId ? $incomingInteraction : null}
      {#if bounds}
        <div
          class="pointer-events-none fixed"
          style:left={`${bounds.x}px`}
          style:top={`${bounds.y}px`}
          style:width={`${bounds.width}px`}
          style:height={`${bounds.height}px`}
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
              class="pointer-events-none absolute -top-5 left-1/2 size-4 -translate-x-1/2 object-contain"
            />
          {/if}

          <Popover
            id={popoverId}
            label="Show live user information"
            labelledBy={`${popoverId}-title`}
            open={selectedUserId === userId}
            onOpenChange={(open) => setPopoverOpen(userId, open)}
            class="size-full"
            triggerClass="scene-hitbox size-full min-h-0"
            panelClass="scene-hitbox fixed w-68"
            panelStyle={popoverPosition(bounds)}
            closeOnOutsidePointer={lockedPopoverUserId !== userId}
            closeOnWindowBlur={lockedPopoverUserId !== userId}
            closeOnFocusOut={lockedPopoverUserId !== userId}
          >
            {#snippet trigger()}
              <span class="block size-full"></span>
            {/snippet}

            <SceneUserPopoverContent
              titleId={`${popoverId}-title`}
              {userId}
              isLocal={userId === $liveMetadata.localId}
              {foregroundApp}
              onModeChange={(active) =>
                (lockedPopoverUserId = active ? userId : null)}
              onDismiss={dismissPopover}
              onSent={dismissPopover}
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
