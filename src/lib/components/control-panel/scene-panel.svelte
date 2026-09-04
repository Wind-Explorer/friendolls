<script lang="ts">
  import { commands, type SceneConfiguration } from "$lib/bindings";
  import {
    sceneConfiguration,
    sceneConfigurationListenerError,
  } from "$lib/listeners/scene-configuration";
  import { profile, profileListenerError } from "$lib/listeners/profile";
  import { onDestroy, onMount } from "svelte";
  import PuppetPreview from "../../../routes/scene/components/renderer/puppet/preview.svelte";
  import PanelMessage from "./panel-message.svelte";
  import type { RegisterPanel } from "./types";
  import { errorMessage, messages } from "$lib/i18n";

  let { register }: { register: RegisterPanel } = $props();

  let configurationDraft = $state<SceneConfiguration>({
    puppetScale: 1,
    puppetOpacity: 1,
    puppetMovementMode: "free",
    hideLocalPuppetWhenAlone: false,
  });
  let skinFile = $state<File | null>(null);
  let skinPreviewUrl = $state<string | null>(null);
  let skinInput = $state<HTMLInputElement | null>(null);
  let pendingSkinReset = $state(false);
  let dirty = $state(false);
  let busy = $state(false);
  let error = $state("");

  $effect(() => {
    if (!dirty) {
      configurationDraft = { ...$sceneConfiguration };
    }
    register("scene", { apply, reset }, { dirty, busy });
  });

  onMount(() => register("scene", { apply, reset }, { dirty, busy }));
  onDestroy(() => {
    if (skinPreviewUrl) URL.revokeObjectURL(skinPreviewUrl);
  });

  function hasConfigurationChanges() {
    return (
      configurationDraft.puppetScale !== $sceneConfiguration.puppetScale ||
      configurationDraft.puppetOpacity !== $sceneConfiguration.puppetOpacity ||
      configurationDraft.puppetMovementMode !==
        $sceneConfiguration.puppetMovementMode ||
      configurationDraft.hideLocalPuppetWhenAlone !==
        $sceneConfiguration.hideLocalPuppetWhenAlone
    );
  }

  function updateDirty() {
    dirty = hasConfigurationChanges() || skinFile !== null || pendingSkinReset;
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
    pendingSkinReset = false;
    skinFile = file;
    skinPreviewUrl = file ? URL.createObjectURL(file) : null;
    updateDirty();
  }

  function resetSkin() {
    clearSkinDraft();
    pendingSkinReset = true;
    updateDirty();
  }

  function reset() {
    configurationDraft = { ...$sceneConfiguration };
    clearSkinDraft();
    pendingSkinReset = false;
    dirty = false;
    error = "";
  }

  async function apply() {
    if (!dirty) return true;

    busy = true;
    error = "";
    try {
      if (pendingSkinReset) {
        await commands.resetProfileSkin();
        pendingSkinReset = false;
      } else if (skinFile) {
        const currentProfile = await commands.getProfile();
        const skinData = Array.from(
          new Uint8Array(await skinFile.arrayBuffer()),
        );
        await commands.updateProfile(currentProfile.displayName, skinData);
        clearSkinDraft();
      }

      if (hasConfigurationChanges()) {
        const configuration = await commands.updateSceneConfiguration({
          ...configurationDraft,
        });
        configurationDraft = { ...configuration };
      }
      dirty = false;
      return true;
    } catch (cause) {
      dirty =
        hasConfigurationChanges() || skinFile !== null || pendingSkinReset;
      error = errorMessage(cause);
      return false;
    } finally {
      busy = false;
    }
  }
</script>

<div class="space-y-1">
  {#if error || $sceneConfigurationListenerError || $profileListenerError}
    <PanelMessage kind="error">
      {errorMessage(
        error || $sceneConfigurationListenerError || $profileListenerError,
      )}
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
                  <p>{$messages.scene_scale()}</p>
                  <p>{$messages.scene_opacity()}</p>
                </div>
                <div class="text-end flex flex-col">
                  <p>{(configurationDraft.puppetScale * 100).toFixed(0)}%</p>
                  <p>{(configurationDraft.puppetOpacity * 100).toFixed(0)}%</p>
                </div>
              </div>
            </div>
            <div class="size-full z-10">
              <PuppetPreview
                userId={$profile.id}
                skinHash={pendingSkinReset ? null : $profile.skinHash}
                skinSource={skinPreviewUrl}
                scale={configurationDraft.puppetScale}
                opacity={configurationDraft.puppetOpacity}
              />
            </div>
          </div>
        {:else}
          <div
            class="skeleton aspect-square size-full"
            aria-label={$messages.scene_loading_preview()}
          ></div>
        {/if}
      </div>
    </div>

    <div class="min-w-0 space-y-2">
      <fieldset class="fieldset border border-base-300 bg-base-100 p-3 pt-1">
        <legend class="fieldset-legend px-1"
          >{$messages.scene_all_puppets()}</legend
        >
        <div>
          <label class="fieldset-label" for="puppet-scale"
            >{$messages.scene_scale()}</label
          >
          <input
            id="puppet-scale"
            class="range range-sm"
            type="range"
            min="0.5"
            max="2"
            step="0.05"
            bind:value={configurationDraft.puppetScale}
            oninput={updateDirty}
            disabled={busy}
            aria-describedby="puppet-scale-help"
          />
        </div>

        <div>
          <label class="fieldset-label" for="puppet-opacity"
            >{$messages.scene_opacity()}</label
          >
          <input
            id="puppet-opacity"
            class="range range-sm"
            type="range"
            min="0.25"
            max="1"
            step="0.05"
            bind:value={configurationDraft.puppetOpacity}
            oninput={updateDirty}
            disabled={busy}
            aria-describedby="puppet-opacity-help"
          />
        </div>

        <label class="label cursor-pointer justify-start gap-2">
          <input
            class="checkbox checkbox-sm checkbox-primary"
            type="checkbox"
            bind:checked={configurationDraft.hideLocalPuppetWhenAlone}
            onchange={updateDirty}
            disabled={busy}
          />
          <span>{$messages.scene_hide_local_puppet_when_alone()}</span>
        </label>
      </fieldset>
      <div class="grid grid-cols-[minmax(0,1fr)_auto] gap-2">
        <button
          class="btn w-full"
          type="button"
          disabled={!$profile || busy}
          onclick={() => skinInput?.click()}
        >
          {$messages.scene_choose_skin()}
        </button>
        <button
          class="btn"
          type="button"
          disabled={!$profile || busy || (!$profile.skinHash && !skinFile)}
          onclick={resetSkin}
        >
          {$messages.common_reset()}
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
    <legend class="fieldset-legend px-1">{$messages.puppet_movement()}</legend>
    <div class="grid grid-cols-2 gap-2">
      <label
        class:border-primary={configurationDraft.puppetMovementMode === "free"}
        class="card cursor-pointer overflow-hidden border border-base-300 bg-base-200"
      >
        <img
          src="/puppet-movement-free.svg"
          alt={$messages.puppet_free_alt()}
          class="w-full object-cover bg-linear-to-b from-base-100 to-base-300"
          class:from-primary-content={configurationDraft.puppetMovementMode ===
            "free"}
        />
        <span
          class="card-body items-center gap-2 p-2 text-center"
          class:bg-primary-content={configurationDraft.puppetMovementMode ===
            "free"}
          class:text-primary={configurationDraft.puppetMovementMode === "free"}
        >
          <span class="text-xs font-medium">{$messages.puppet_free()}</span>
          <input
            class="hidden"
            type="radio"
            name="puppet-movement-mode"
            value="free"
            checked={configurationDraft.puppetMovementMode === "free"}
            onchange={() => {
              configurationDraft.puppetMovementMode = "free";
              updateDirty();
            }}
            disabled={busy}
          />
        </span>
      </label>

      <label
        class:border-primary={configurationDraft.puppetMovementMode ===
          "bottom"}
        class:text-primary={configurationDraft.puppetMovementMode === "bottom"}
        class="card cursor-pointer overflow-hidden border border-base-300 bg-base-200"
      >
        <img
          src="/puppet-movement-bottom.svg"
          alt={$messages.puppet_bottom_alt()}
          class="w-full object-cover bg-linear-to-b from-base-100 to-base-300"
          class:from-primary-content={configurationDraft.puppetMovementMode ===
            "bottom"}
        />
        <span
          class="card-body items-center gap-2 p-2 text-center"
          class:bg-primary-content={configurationDraft.puppetMovementMode ===
            "bottom"}
        >
          <span class="text-xs font-medium">{$messages.puppet_bottom()}</span>
          <input
            class="hidden"
            type="radio"
            name="puppet-movement-mode"
            value="bottom"
            checked={configurationDraft.puppetMovementMode === "bottom"}
            onchange={() => {
              configurationDraft.puppetMovementMode = "bottom";
              updateDirty();
            }}
            disabled={busy}
          />
        </span>
      </label>
    </div>
  </fieldset>
</div>
