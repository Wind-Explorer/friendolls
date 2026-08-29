<script lang="ts">
  import { commands } from "$lib/bindings";
  import { profile, profileListenerError } from "$lib/listeners/profile";
  import { onMount } from "svelte";
  import PanelMessage from "./panel-message.svelte";
  import type { RegisterPanel } from "./types";
  import Info from "$lib/icons/info.svelte";

  let { register }: { register: RegisterPanel } = $props();

  let displayName = $state("");
  let dirty = $state(false);
  let busy = $state(false);
  let error = $state("");

  $effect(() => {
    if (!dirty) displayName = $profile?.displayName ?? "";
    register("general", { apply, reset }, { dirty, busy });
  });

  onMount(() => register("general", { apply, reset }, { dirty, busy }));

  function updateDirty() {
    dirty = displayName.trim() !== ($profile?.displayName ?? "");
    error = "";
  }

  function reset() {
    displayName = $profile?.displayName ?? "";
    dirty = false;
    error = "";
  }

  async function apply() {
    const nextName = displayName.trim();
    if (!dirty) return true;
    if (!nextName) {
      error = "Display name cannot be empty.";
      return false;
    }

    busy = true;
    error = "";
    try {
      await commands.updateProfile(nextName, null);
      displayName = nextName;
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
  {#if error || $profileListenerError}
    <PanelMessage kind="error">{error || $profileListenerError}</PanelMessage>
  {/if}

  <fieldset class="fieldset border border-base-300 bg-base-100 p-3">
    <legend class="fieldset-legend px-1">Identity</legend>

    <label class="fieldset-label" for="display-name">Display name</label>
    <input
      id="display-name"
      class="input w-full"
      bind:value={displayName}
      oninput={updateDirty}
      maxlength="64"
      autocomplete="off"
      disabled={!$profile || busy}
      aria-describedby="display-name-help"
    />
    <p id="display-name-help" class="fieldset-label">
      This name can be changed at any time.
    </p>

    <label class="fieldset-label mt-2" for="public-key"
      >Identification Key
      <div
        class="tooltip tooltip-primary"
        data-tip="Share only with people you trust."
      >
        <div class="*:size-3">
          <Info />
        </div>
      </div>
    </label>
    {#if $profile}
      <div class="join w-full">
        <input
          id="public-key"
          class="input join-item min-w-0 flex-1 font-mono text-[10px]"
          value={$profile.id}
          readonly
          aria-label="Public key"
        />
        <button
          class="btn join-item"
          type="button"
          onclick={() => navigator.clipboard.writeText($profile?.id ?? "")}
          title="Copy public key">Copy</button
        >
      </div>
    {:else if $profileListenerError}
      <input
        id="public-key"
        class="input w-full"
        value="Unavailable"
        disabled
      />
    {:else}
      <div class="skeleton h-7 w-full" aria-label="Loading public key"></div>
    {/if}
  </fieldset>
</div>
