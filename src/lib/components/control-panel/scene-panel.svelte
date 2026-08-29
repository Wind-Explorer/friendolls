<script lang="ts">
  import { commands } from "$lib/bindings";
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
  let dirty = $state(false);
  let busy = $state(false);
  let error = $state("");

  $effect(() => {
    if (!dirty) {
      puppetScale = $sceneConfiguration.puppetScale;
      puppetOpacity = $sceneConfiguration.puppetOpacity;
    }
    register("scene", { apply, reset }, { dirty, busy });
  });

  onMount(() => register("scene", { apply, reset }, { dirty, busy }));

  function updateDirty() {
    dirty =
      puppetScale !== $sceneConfiguration.puppetScale ||
      puppetOpacity !== $sceneConfiguration.puppetOpacity;
    error = "";
  }

  function reset() {
    puppetScale = $sceneConfiguration.puppetScale;
    puppetOpacity = $sceneConfiguration.puppetOpacity;
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
      });
      puppetScale = configuration.puppetScale;
      puppetOpacity = configuration.puppetOpacity;
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
  </fieldset>
</div>
