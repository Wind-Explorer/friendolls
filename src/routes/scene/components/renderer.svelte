<script lang="ts">
  import type { PuppetState } from "$lib/bindings";
  import { sceneConfiguration } from "$lib/listeners/scene-configuration";
  import { onDestroy, onMount } from "svelte";
  import { SceneScheduler } from "./renderer/scheduler";
  import { PuppetManager } from "./renderer/puppet/manager";
  import type { PuppetScreenBounds, SceneRenderInputs } from "./renderer/types";
  import { World } from "./renderer/world";

  let {
    puppets,
    selectedPuppetId,
    onBoundsChange,
    skinHashes,
  }: {
    puppets: readonly PuppetState[];
    selectedPuppetId: string | null;
    onBoundsChange: (bounds: PuppetScreenBounds[]) => void;
    skinHashes: ReadonlyMap<string, string | null>;
  } = $props();

  let renderDiv = $state<HTMLDivElement | null>(null);
  let renderInputs: SceneRenderInputs = $derived({
    puppets,
    scale: $sceneConfiguration.puppetScale,
    idleOpacity: $sceneConfiguration.puppetOpacity,
    selectedPuppetId,
    skinHashes,
  });

  let world: World | null = null;
  let puppetManager: PuppetManager | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let scheduler: SceneScheduler | null = null;
  let previousBounds: PuppetScreenBounds[] = [];
  let boundsPuppetIds = new Set<string>();
  let lastBoundsUpdate = -Infinity;

  function animate(deltaSeconds: number, elapsedSeconds: number) {
    if (!world || !puppetManager) return false;

    const active = puppetManager.update(
      renderInputs,
      deltaSeconds,
      elapsedSeconds,
    );
    world.render();

    const membershipChanged =
      boundsPuppetIds.size !== renderInputs.puppets.length ||
      renderInputs.puppets.some(({ id }) => !boundsPuppetIds.has(id));
    // Always flush the final pose; a sleeping scene has no later frame to do it.
    if (
      !active ||
      membershipChanged ||
      elapsedSeconds - lastBoundsUpdate >= 1 / 20
    ) {
      lastBoundsUpdate = elapsedSeconds;
      const nextBounds = puppetManager.screenBounds();
      boundsPuppetIds = new Set(nextBounds.map(({ id }) => id));
      if (!sameBounds(previousBounds, nextBounds)) {
        previousBounds = nextBounds;
        onBoundsChange(nextBounds);
      }
    }

    return active;
  }

  $effect(() => {
    void renderInputs;
    scheduler?.invalidate();
  });

  function syncVisibility() {
    scheduler?.setSuspended(document.hidden);
  }

  function sameBounds(
    previous: PuppetScreenBounds[],
    next: PuppetScreenBounds[],
  ) {
    if (previous.length !== next.length) return false;
    return previous.every((bounds, index) => {
      const candidate = next[index];
      return (
        candidate?.id === bounds.id &&
        Math.abs(candidate.x - bounds.x) < 0.25 &&
        Math.abs(candidate.y - bounds.y) < 0.25 &&
        Math.abs(candidate.width - bounds.width) < 0.25 &&
        Math.abs(candidate.height - bounds.height) < 0.25
      );
    });
  }

  onMount(() => {
    if (!renderDiv) return;

    world = new World(renderDiv);
    scheduler = new SceneScheduler(animate);
    puppetManager = new PuppetManager(world, scheduler.invalidate);

    resizeObserver = new ResizeObserver(() => {
      world?.resizeWorld();
      lastBoundsUpdate = -Infinity;
      scheduler?.invalidate();
    });
    resizeObserver.observe(renderDiv);

    document.addEventListener("visibilitychange", syncVisibility);
    syncVisibility();
  });

  onDestroy(() => {
    scheduler?.dispose();
    document.removeEventListener("visibilitychange", syncVisibility);
    resizeObserver?.disconnect();
    puppetManager?.dispose();
    world?.dispose();
    onBoundsChange([]);
  });
</script>

<div
  class="pointer-events-none absolute inset-0 overflow-hidden"
  aria-hidden="true"
  bind:this={renderDiv}
></div>
