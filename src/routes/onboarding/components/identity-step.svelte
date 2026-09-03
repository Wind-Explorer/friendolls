<script lang="ts">
  import { messages } from "$lib/i18n";
  let { displayName = $bindable(), busy }: { displayName: string; busy: boolean } = $props();
</script>

<fieldset class="fieldset border border-base-300 bg-base-100 p-4">
  <legend class="fieldset-legend px-1">{$messages.onboarding_identity_legend()}</legend>
  <label class="label" for="onboarding-display-name">{$messages.common_display_name()}</label>
  <input
    id="onboarding-display-name"
    class="input w-full"
    bind:value={displayName}
    maxlength="64"
    autocomplete="off"
    disabled={busy}
  />
  <p class="label">{$messages.onboarding_identity_help()}</p>
</fieldset>
