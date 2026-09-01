<script lang="ts">
  let { displayName = $bindable(), busy }: { displayName: string; busy: boolean } = $props();
</script>

<fieldset class="fieldset border border-base-300 bg-base-100 p-4">
  <legend class="fieldset-legend px-1">Identity</legend>
  <label class="label" for="onboarding-display-name">Display name</label>
  <input
    id="onboarding-display-name"
    class="input w-full"
    bind:value={displayName}
    maxlength="64"
    autocomplete="off"
    disabled={busy}
  />
  <p class="label">Up to 64 characters. You can change it later.</p>
</fieldset>
