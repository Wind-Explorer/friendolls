<script lang="ts">
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";
  import ServerEndpointFields from "$lib/components/server-endpoint-fields.svelte";
  import { messages } from "$lib/i18n";
  import type { ServerConnectionType } from "$lib/server-endpoint";

  let {
    serverName = $bindable(),
    serverAddress = $bindable(),
    serverPort = $bindable(),
    serverConnectionType = $bindable(),
    busy,
  }: {
    serverName: string;
    serverAddress: string;
    serverPort: string;
    serverConnectionType: ServerConnectionType;
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
  <ServerEndpointFields
    idPrefix="onboarding-server"
    bind:address={serverAddress}
    bind:port={serverPort}
    bind:connectionType={serverConnectionType}
    disabled={busy}
    labelClass="label"
  />
</fieldset>
