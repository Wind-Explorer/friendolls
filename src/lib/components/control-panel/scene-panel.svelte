<script lang="ts">
  import { commands, type PuppetMovementMode } from "$lib/bindings";
  import {
    sceneConfiguration,
    sceneConfigurationListenerError,
  } from "$lib/listeners/scene-configuration";
  import { profile, profileListenerError } from "$lib/listeners/profile";
  import { onDestroy, onMount } from "svelte";
  import PuppetPreview from "../../../routes/scene/components/renderer/puppet/preview.svelte";
  import PanelMessage from "./panel-message.svelte";
  import type { RegisterPanel } from "./types";

  let { register }: { register: RegisterPanel } = $props();

  let puppetScale = $state(1);
  let puppetOpacity = $state(1);
  let puppetMovementMode = $state<PuppetMovementMode>("free");
  let skinFile = $state<File | null>(null);
  let skinPreviewUrl = $state<string | null>(null);
  let skinInput = $state<HTMLInputElement | null>(null);
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
  onDestroy(() => {
    if (skinPreviewUrl) URL.revokeObjectURL(skinPreviewUrl);
  });

  function hasConfigurationChanges() {
    return (
      puppetScale !== $sceneConfiguration.puppetScale ||
      puppetOpacity !== $sceneConfiguration.puppetOpacity ||
      puppetMovementMode !== $sceneConfiguration.puppetMovementMode
    );
  }

  function updateDirty() {
    dirty = hasConfigurationChanges() || skinFile !== null;
    error = "";
  }

  function clearSkinDraft() {
    if (skinPreviewUrl) URL.revokeObjectURL(skinPreviewUrl);
    skinFile = null;
    skinPreviewUrl = null;
    if (skinInput) skinInput.value = "";
  }

  function selectSkinFile(file: File | null) {
    clearSkinDraft();
    skinFile = file;
    skinPreviewUrl = file ? URL.createObjectURL(file) : null;
    updateDirty();
  }

  function reset() {
    puppetScale = $sceneConfiguration.puppetScale;
    puppetOpacity = $sceneConfiguration.puppetOpacity;
    puppetMovementMode = $sceneConfiguration.puppetMovementMode;
    clearSkinDraft();
    dirty = false;
    error = "";
  }

  async function apply() {
    if (!dirty) return true;

    busy = true;
    error = "";
    try {
      if (skinFile) {
        const currentProfile = await commands.getProfile();
        const skinData = Array.from(
          new Uint8Array(await skinFile.arrayBuffer()),
        );
        await commands.updateProfile(currentProfile.displayName, skinData);
        clearSkinDraft();
      }

      if (hasConfigurationChanges()) {
        const configuration = await commands.updateSceneConfiguration({
          puppetScale,
          puppetOpacity,
          puppetMovementMode,
        });
        puppetScale = configuration.puppetScale;
        puppetOpacity = configuration.puppetOpacity;
        puppetMovementMode = configuration.puppetMovementMode;
      }
      dirty = false;
      return true;
    } catch (cause) {
      dirty = hasConfigurationChanges() || skinFile !== null;
      error = String(cause);
      return false;
    } finally {
      busy = false;
    }
  }
</script>

<div class="space-y-1">
  {#if error || $sceneConfigurationListenerError || $profileListenerError}
    <PanelMessage kind="error">
      {error || $sceneConfigurationListenerError || $profileListenerError}
    </PanelMessage>
  {/if}

  <div class="grid grid-cols-[minmax(0,3fr)_minmax(0,4fr)] items-center gap-3">
    <div class="flex flex-col gap-2">
      <div class="aspect-square min-w-0 mt-2">
        {#if $profile}
          <div
            class="border border-primary relative shadow-[inset_0_0_10px] bg-primary/5 shadow-primary card"
          >
            <div class="size-full absolute bg-gridded opacity-25 inset-0"></div>
            <div class="size-full absolute">
              <div
                class="flex flex-row size-full items-end justify-between text-[10px] text-primary p-1"
              >
                <div class="text-start flex flex-col">
                  <p>Scale</p>
                  <p>Opacity</p>
                </div>
                <div class="text-end flex flex-col">
                  <p>{(puppetScale * 100).toFixed(0)}%</p>
                  <p>{(puppetOpacity * 100).toFixed(0)}%</p>
                </div>
              </div>
            </div>
            <div class="size-full z-10">
              <PuppetPreview
                userId={$profile.id}
                skinHash={$profile.skinHash}
                skinSource={skinPreviewUrl}
                scale={puppetScale}
                opacity={puppetOpacity}
              />
            </div>
          </div>
        {:else}
          <div
            class="skeleton aspect-square size-full"
            aria-label="Loading puppet preview"
          ></div>
        {/if}
      </div>
    </div>

    <div class="min-w-0 space-y-2">
      <fieldset class="fieldset border border-base-300 bg-base-100 p-3 pt-1">
        <legend class="fieldset-legend px-1">All puppets</legend>
        <div>
          <label class="fieldset-label" for="puppet-scale">Scale</label>
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
        </div>

        <div>
          <label class="fieldset-label" for="puppet-opacity">Opacity</label>
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
        </div>
      </fieldset>
      <div>
        <button
          class="btn w-full"
          type="button"
          disabled={!$profile || busy}
          onclick={() => skinInput?.click()}
        >
          Choose a custom skin
        </button>
        <input
          id="skin-file"
          class="hidden"
          type="file"
          accept="image/png"
          bind:this={skinInput}
          onchange={(event) =>
            selectSkinFile(event.currentTarget.files?.[0] ?? null)}
        />
      </div>
    </div>
  </div>
  <fieldset class="fieldset border border-base-300 bg-base-100 p-3 pt-1">
    <legend class="fieldset-legend px-1">Puppets reposition</legend>
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
</div>

<style>
  .bg-gridded {
    background-image:
      linear-gradient(
        0deg,
        transparent 24%,
        var(--color-primary) 25%,
        var(--color-primary) 26%,
        transparent 27%,
        transparent 74%,
        var(--color-primary) 75%,
        var(--color-primary) 76%,
        transparent 77%,
        transparent
      ),
      linear-gradient(
        90deg,
        transparent 24%,
        var(--color-primary) 25%,
        var(--color-primary) 26%,
        transparent 27%,
        transparent 74%,
        var(--color-primary) 75%,
        var(--color-primary) 76%,
        transparent 77%,
        transparent
      );
    background-size: 32px 32px;
  }
</style>
