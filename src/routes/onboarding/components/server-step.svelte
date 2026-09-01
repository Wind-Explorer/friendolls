<script lang="ts">
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";

  let {
    serverName = $bindable(),
    serverAddress = $bindable(),
    serverPort = $bindable(),
    busy,
  }: {
    serverName: string;
    serverAddress: string;
    serverPort: string;
    busy: boolean;
  } = $props();
</script>

<PanelMessage
  >If you do not have a server yet, leave this empty and continue. You can add
  one later.</PanelMessage
>
<fieldset class="fieldset mt-3 border border-base-300 bg-base-100 p-4">
  <legend class="fieldset-legend px-1">Server details</legend>
  <label class="label" for="onboarding-server-name"
    >Friendly name (optional)</label
  >
  <input
    id="onboarding-server-name"
    class="input w-full"
    bind:value={serverName}
    maxlength="64"
    autocomplete="off"
    disabled={busy}
  />
  <label class="label mt-1" for="onboarding-server-address">Address</label>
  <input
    id="onboarding-server-address"
    class="input w-full"
    bind:value={serverAddress}
    placeholder="example.net"
    autocomplete="off"
    disabled={busy}
  />
  <label class="label mt-1" for="onboarding-server-port">Port (optional)</label>
  <input
    id="onboarding-server-port"
    class="input w-28"
    bind:value={serverPort}
    type="number"
    min="1"
    max="65535"
    inputmode="numeric"
    placeholder="Default"
    disabled={busy}
  />
</fieldset>
