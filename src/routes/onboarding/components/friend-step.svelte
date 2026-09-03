<script lang="ts">
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import { messages } from "$lib/i18n";

  let { friendId = $bindable(), busy }: { friendId: string; busy: boolean } =
    $props();
</script>

<PanelMessage>{$messages.onboarding_friend_skip()}</PanelMessage>
<fieldset class="fieldset mt-3 border border-base-300 bg-base-100 p-4">
  <legend class="fieldset-legend px-1"
    >{$messages.common_friend_identity()}</legend
  >
  <label class="label" for="onboarding-friend-key"
    >{$messages.common_identification_key()}</label
  >
  <textarea
    id="onboarding-friend-key"
    class="textarea h-24 w-full resize-none text-[10px]"
    bind:value={friendId}
    autocomplete="off"
    disabled={busy}></textarea>
  <p class="label">{$messages.friend_key_verify()}</p>
</fieldset>
