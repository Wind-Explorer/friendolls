<script lang="ts">
  import {
    errorMessage,
    languageOptions,
    localePreference,
    messages,
    setLanguagePreference,
  } from "$lib/i18n";
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";

  let {
    requiresAccessibilityPermission,
  }: { requiresAccessibilityPermission: boolean } = $props();

  let busy = $state(false);
  let error = $state("");

  async function changeLanguage(event: Event) {
    busy = true;
    try {
      error = "";
      await setLanguagePreference(
        (event.currentTarget as HTMLSelectElement).value,
      );
    } catch (cause) {
      error = errorMessage(cause);
    } finally {
      busy = false;
    }
  }
</script>

<div class="space-y-4 text-sm">
  <p>{$messages.onboarding_welcome_intro()}</p>
  <p>{$messages.onboarding_welcome_description()}</p>
  {#if error}<PanelMessage kind="error">{error}</PanelMessage>{/if}
  <fieldset class="fieldset border border-base-300 bg-base-100 p-3">
    <legend class="fieldset-legend px-1">{$messages.language_label()}</legend>
    <select
      class="select w-full"
      aria-label={$messages.language_label()}
      value={$localePreference}
      disabled={busy}
      onchange={changeLanguage}
    >
      {#each languageOptions as option (option.value)}
        <option value={option.value}>{option.label($messages)}</option>
      {/each}
    </select>
  </fieldset>
</div>
