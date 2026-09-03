<script lang="ts">
  import { commands } from "$lib/bindings";
  import { profile, profileListenerError } from "$lib/listeners/profile";
  import { onMount } from "svelte";
  import PanelMessage from "./panel-message.svelte";
  import type { RegisterPanel } from "./types";
  import Info from "$lib/icons/info.svelte";
  import {
    errorMessage,
    languageOptions,
    localePreference,
    messages,
    setLanguagePreference,
  } from "$lib/i18n";

  let { register }: { register: RegisterPanel } = $props();

  let displayName = $state("");
  let dirty = $state(false);
  let busy = $state(false);
  let error = $state("");
  let languageBusy = $state(false);

  async function changeLanguage(event: Event) {
    languageBusy = true;
    error = "";
    try {
      await setLanguagePreference(
        (event.currentTarget as HTMLSelectElement).value,
      );
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      languageBusy = false;
    }
  }

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
      error = $messages.error_display_name_empty();
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
      error = errorMessage(cause);
      return false;
    } finally {
      busy = false;
    }
  }
</script>

<div class="space-y-3">
  {#if error || $profileListenerError}
    <PanelMessage kind="error"
      >{errorMessage(error || $profileListenerError)}</PanelMessage
    >
  {/if}

  <fieldset class="fieldset border border-base-300 bg-base-100 p-3">
    <legend class="fieldset-legend px-1">{$messages.account_identity()}</legend>

    <label class="fieldset-label" for="display-name"
      >{$messages.common_display_name()}</label
    >
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
      {$messages.account_name_help()}
    </p>

    <label class="fieldset-label mt-2" for="public-key"
      >{$messages.common_identification_key()}
      <div
        class="tooltip tooltip-primary"
        data-tip={$messages.account_key_trust()}
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
          class="input join-item min-w-0 flex-1 text-[10px]"
          value={$profile.id}
          readonly
          aria-label={$messages.account_public_key()}
        />
        <button
          class="btn join-item"
          type="button"
          onclick={() => navigator.clipboard.writeText($profile?.id ?? "")}
          title={$messages.account_copy_key()}>{$messages.common_copy()}</button
        >
      </div>
    {:else if $profileListenerError}
      <input
        id="public-key"
        class="input w-full"
        value={$messages.common_unavailable()}
        disabled
      />
    {:else}
      <div
        class="skeleton h-7 w-full"
        aria-label={$messages.account_loading_key()}
      ></div>
    {/if}
  </fieldset>

  <fieldset class="fieldset border border-base-300 bg-base-100 p-3">
    <legend class="fieldset-legend px-1">{$messages.account_ui_label()}</legend>
    <label class="fieldset-label" for="language"
      >{$messages.language_label()}</label
    >
    <select
      id="language"
      class="select w-full"
      value={$localePreference}
      disabled={languageBusy}
      onchange={changeLanguage}
    >
      {#each languageOptions as option (option.value)}
        <option value={option.value}>{option.label($messages)}</option>
      {/each}
    </select>
  </fieldset>
</div>
