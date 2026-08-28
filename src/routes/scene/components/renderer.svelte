<script lang="ts">
  import type { PuppetState } from "$lib/bindings";
  import { onDestroy, onMount } from "svelte";
  import * as THREE from "three";
  import { PuppetManager } from "./renderer/puppet/manager";
  import type { PuppetScreenBounds } from "./renderer/types";
  import { World } from "./renderer/world";

  let {
    puppets,
    frozenPuppetId,
    onBoundsChange,
    skinHashes,
  }: {
    puppets: readonly PuppetState[];
    frozenPuppetId: string | null;
    onBoundsChange: (bounds: PuppetScreenBounds[]) => void;
    skinHashes: ReadonlyMap<string, string | null>;
  } = $props();

  let renderDiv = $state<HTMLDivElement | null>(null);

  let world: World | null = null;
  let puppetManager: PuppetManager | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let clock: THREE.Timer | null = null;
  let animationFrameId = 0;
  let lastBoundsUpdate = 0;
  let previousBounds: PuppetScreenBounds[] = [];

  function animate() {
    if (!world || !puppetManager || !clock) return;

    animationFrameId = requestAnimationFrame(animate);

    clock.update();
    puppetManager.update(
      puppets,
      frozenPuppetId,
      clock.getDelta(),
      clock.getElapsed(),
      skinHashes,
    );
    world.render();

    const elapsed = clock.getElapsed();
    if (elapsed - lastBoundsUpdate >= 1 / 30) {
      lastBoundsUpdate = elapsed;
      const nextBounds = puppetManager.screenBounds();
      if (!sameBounds(previousBounds, nextBounds)) {
        previousBounds = nextBounds;
        onBoundsChange(nextBounds);
      }
    }
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
    puppetManager = new PuppetManager(world);
    clock = new THREE.Timer();
    clock.connect(document);

    resizeObserver = new ResizeObserver(world.resizeWorld);
    resizeObserver.observe(renderDiv);

    animate();
  });

  onDestroy(() => {
    cancelAnimationFrame(animationFrameId);
    resizeObserver?.disconnect();
    clock?.dispose();
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
