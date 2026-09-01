<script lang="ts">
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";

  let { friendId = $bindable(), busy }: { friendId: string; busy: boolean } =
    $props();
</script>

<PanelMessage
  >If no one has shared a key with you yet, leave this empty and continue. You
  can add friends anytime afterwards.</PanelMessage
>
<fieldset class="fieldset mt-3 border border-base-300 bg-base-100 p-4">
  <legend class="fieldset-legend px-1">Friend identity</legend>
  <label class="label" for="onboarding-friend-key">Identification Key</label>
  <textarea
    id="onboarding-friend-key"
    class="textarea h-24 w-full resize-none font-mono text-[10px]"
    bind:value={friendId}
    autocomplete="off"
    disabled={busy}></textarea>
  <p class="label">Verify this key with your friend before adding it.</p>
</fieldset>
