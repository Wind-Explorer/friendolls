<script lang="ts">
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import { messages } from "$lib/i18n";

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

<PanelMessage>{$messages.onboarding_server_skip()}</PanelMessage>
<fieldset class="fieldset mt-3 border border-base-300 bg-base-100 p-4">
  <legend class="fieldset-legend px-1">{$messages.common_server_details()}</legend>
  <label class="label" for="onboarding-server-name"
    >{$messages.common_optional_friendly_name()}</label
  >
  <input
    id="onboarding-server-name"
    class="input w-full"
    bind:value={serverName}
    maxlength="64"
    autocomplete="off"
    disabled={busy}
  />
  <label class="label mt-1" for="onboarding-server-address">{$messages.common_address()}</label>
  <input
    id="onboarding-server-address"
    class="input w-full"
    bind:value={serverAddress}
    placeholder="example.net"
    autocomplete="off"
    disabled={busy}
  />
  <label class="label mt-1" for="onboarding-server-port">{$messages.common_optional_port()}</label>
  <input
    id="onboarding-server-port"
    class="input w-28"
    bind:value={serverPort}
    type="number"
    min="1"
    max="65535"
    inputmode="numeric"
    placeholder="27520"
    disabled={busy}
  />
</fieldset>
