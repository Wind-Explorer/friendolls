<script lang="ts">
  import type { SceneConfiguration, User } from "$lib/bindings";
  import PuppetPreview from "../../scene/components/renderer/puppet/preview.svelte";
  import { messages } from "$lib/i18n";

  let {
    profile,
    configuration,
    skinMode = $bindable(),
    skinPreviewUrl,
    busy,
    onselect,
  }: {
    profile: User;
    configuration: SceneConfiguration;
    skinMode: "current" | "default" | "custom";
    skinPreviewUrl: string | null;
    busy: boolean;
    onselect: (file: File | null) => void;
  } = $props();
</script>

<div class="grid grid-cols-[9rem_minmax(0,1fr)] gap-4 items-center">
  <div class="bg-gridded aspect-square">
    <div
      class="aspect-square border border-primary bg-primary/5 shadow-[inset_0_0_10px] shadow-primary"
    >
      <PuppetPreview
        userId={profile.id}
        skinHash={skinMode === "default" ? null : profile.skinHash}
        skinSource={skinMode === "custom" ? skinPreviewUrl : null}
        scale={configuration.puppetScale}
        opacity={configuration.puppetOpacity}
      />
    </div>
  </div>
  <fieldset class="fieldset border border-base-300 bg-base-100 p-4">
    <legend class="fieldset-legend px-1">{$messages.onboarding_skin_legend()}</legend>
    {#if profile.skinHash}
      <label class="label cursor-pointer justify-start gap-2">
        <input
          class="radio radio-sm radio-primary"
          type="radio"
          name="skin-mode"
          value="current"
          bind:group={skinMode}
          disabled={busy}
        />
        {$messages.onboarding_skin_keep()}
      </label>
    {/if}
    <label class="label cursor-pointer justify-start gap-2">
      <input
        class="radio radio-sm radio-primary"
        type="radio"
        name="skin-mode"
        value="default"
        bind:group={skinMode}
        disabled={busy}
      />
      {$messages.onboarding_skin_default()}
    </label>
    <label class="label cursor-pointer justify-start gap-2">
      <input
        class="radio radio-sm radio-primary"
        type="radio"
        name="skin-mode"
        value="custom"
        bind:group={skinMode}
        disabled={busy}
      />
      {$messages.onboarding_skin_choose_png()}
    </label>
    <input
      class="file-input file-input-sm mt-2 w-full"
      type="file"
      accept="image/png"
      disabled={busy}
      onchange={(event) => onselect(event.currentTarget.files?.[0] ?? null)}
    />
    <p class="label">{$messages.onboarding_skin_help()}</p>
  </fieldset>
</div>
