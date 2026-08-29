<script lang="ts">
  import { commands, type PuppetMovementMode } from "$lib/bindings";
  import {
    sceneConfiguration,
    sceneConfigurationListenerError,
  } from "$lib/listeners/scene-configuration";
  import { onMount } from "svelte";
  import PanelMessage from "./panel-message.svelte";
  import type { RegisterPanel } from "./types";

  let { register }: { register: RegisterPanel } = $props();

  let puppetScale = $state(1);
  let puppetOpacity = $state(1);
  let puppetMovementMode = $state<PuppetMovementMode>("free");
  let dirty = $state(false);
  let busy = $state(false);
  let error = $state("");

  $effect(() => {
    if (!dirty) {
      puppetScale = $sceneConfiguration.puppetScale;
      puppetOpacity = $sceneConfiguration.puppetOpacity;
      puppetMovementMode = $sceneConfiguration.puppetMovementMode;
    }
    register("scene", { apply, reset }, { dirty, busy });
  });

  onMount(() => register("scene", { apply, reset }, { dirty, busy }));

  function updateDirty() {
    dirty =
      puppetScale !== $sceneConfiguration.puppetScale ||
      puppetOpacity !== $sceneConfiguration.puppetOpacity ||
      puppetMovementMode !== $sceneConfiguration.puppetMovementMode;
    error = "";
  }

  function reset() {
    puppetScale = $sceneConfiguration.puppetScale;
    puppetOpacity = $sceneConfiguration.puppetOpacity;
    puppetMovementMode = $sceneConfiguration.puppetMovementMode;
    dirty = false;
    error = "";
  }

  async function apply() {
    if (!dirty) return true;

    busy = true;
    error = "";
    try {
      const configuration = await commands.updateSceneConfiguration({
        puppetScale,
        puppetOpacity,
        puppetMovementMode,
      });
      puppetScale = configuration.puppetScale;
      puppetOpacity = configuration.puppetOpacity;
      puppetMovementMode = configuration.puppetMovementMode;
      dirty = false;
      return true;
    } catch (cause) {
      error = String(cause);
      return false;
    } finally {
      busy = false;
    }
  }
</script>

<div class="space-y-3">
  {#if error || $sceneConfigurationListenerError}
    <PanelMessage kind="error">
      {error || $sceneConfigurationListenerError}
    </PanelMessage>
  {/if}

  <fieldset class="fieldset border border-base-300 bg-base-100 p-3">
    <legend class="fieldset-legend px-1">Puppets</legend>

    <div class="flex items-center justify-between gap-3">
      <label class="fieldset-label" for="puppet-scale">Scale</label>
      <output class="badge badge-outline tabular-nums" for="puppet-scale">
        {puppetScale.toFixed(2)}×
      </output>
    </div>
    <input
      id="puppet-scale"
      class="range range-sm"
      type="range"
      min="0.5"
      max="2"
      step="0.05"
      bind:value={puppetScale}
      oninput={updateDirty}
      disabled={busy}
      aria-describedby="puppet-scale-help"
    />

    <div class="mt-3 flex items-center justify-between gap-3">
      <label class="fieldset-label" for="puppet-opacity">Opacity</label>
      <output class="badge badge-outline tabular-nums" for="puppet-opacity">
        {Math.round(puppetOpacity * 100)}%
      </output>
    </div>
    <input
      id="puppet-opacity"
      class="range range-sm"
      type="range"
      min="0.25"
      max="1"
      step="0.05"
      bind:value={puppetOpacity}
      oninput={updateDirty}
      disabled={busy}
      aria-describedby="puppet-opacity-help"
    />

    <fieldset class="mt-4">
      <legend class="fieldset-label mb-2">Movement</legend>
      <div class="grid grid-cols-2 gap-2">
        <label
          class:border-primary={puppetMovementMode === "free"}
          class="card cursor-pointer overflow-hidden border border-base-300 bg-base-200"
        >
          <img
            src="/puppet-movement-free.svg"
            alt="Puppets moving freely across the full viewport"
            class="w-full object-cover bg-linear-to-b from-base-100 to-base-300"
            class:from-primary-content={puppetMovementMode === "free"}
          />
          <span
            class="card-body items-center gap-2 p-2 text-center"
            class:bg-primary-content={puppetMovementMode === "free"}
            class:text-primary={puppetMovementMode === "free"}
          >
            <span class="text-xs font-medium">Freeroam around</span>
            <input
              class="hidden"
              type="radio"
              name="puppet-movement-mode"
              value="free"
              checked={puppetMovementMode === "free"}
              onchange={() => {
                puppetMovementMode = "free";
                updateDirty();
              }}
              disabled={busy}
            />
          </span>
        </label>

        <label
          class:border-primary={puppetMovementMode === "bottom"}
          class:text-primary={puppetMovementMode === "bottom"}
          class="card cursor-pointer overflow-hidden border border-base-300 bg-base-200"
        >
          <img
            src="/puppet-movement-bottom.svg"
            alt="Puppets moving along the bottom of the viewport"
            class="w-full object-cover bg-linear-to-b from-base-100 to-base-300"
            class:from-primary-content={puppetMovementMode === "bottom"}
          />
          <span
            class="card-body items-center gap-2 p-2 text-center"
            class:bg-primary-content={puppetMovementMode === "bottom"}
          >
            <span class="text-xs font-medium">Stay on the ground</span>
            <input
              class="hidden"
              type="radio"
              name="puppet-movement-mode"
              value="bottom"
              checked={puppetMovementMode === "bottom"}
              onchange={() => {
                puppetMovementMode = "bottom";
                updateDirty();
              }}
              disabled={busy}
            />
          </span>
        </label>
      </div>
    </fieldset>
  </fieldset>
</div>
