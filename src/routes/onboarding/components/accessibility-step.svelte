<script lang="ts">
  import PanelMessage from "$lib/components/control-panel/panel-message.svelte";

  let {
    permissionGranted,
    busy,
    onrequest,
  }: { permissionGranted: boolean; busy: boolean; onrequest: () => void } =
    $props();
</script>

<div class="space-y-4">
  <PanelMessage kind={permissionGranted ? "success" : "warning"}>
    {permissionGranted
      ? "Accessibility access is granted. Puppets can now follow your cursor."
      : "Accessibility access is still waiting for your approval."}
  </PanelMessage>
  <fieldset class="fieldset border border-base-300 bg-base-100 p-4">
    <legend class="fieldset-legend px-1">Why Friendolls needs this</legend>
    <p class="text-xs leading-relaxed">
      Friendolls reads the system cursor position so your puppet can follow you
      and your friends can see that movement. It does not use Accessibility
      access to read typed text, click buttons, or control other apps.
    </p>
    <ol class="mt-3 list-decimal space-y-1 pl-5 text-xs">
      <li>Click <strong>Open Accessibility Settings</strong>.</li>
      <li>Turn on Friendolls in Privacy &amp; Security → Accessibility.</li>
      <li>Return here; this wizard confirms the change automatically.</li>
    </ol>
    <button
      class="btn mt-4 w-fit"
      type="button"
      disabled={busy || permissionGranted}
      onclick={onrequest}
    >
      {permissionGranted ? "Access Granted" : "Open Accessibility Settings…"}
    </button>
  </fieldset>
</div>
