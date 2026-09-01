<script lang="ts">
  import type { SceneConfiguration, User } from "$lib/bindings";
  import PuppetPreview from "../../scene/components/renderer/puppet/preview.svelte";

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
    <legend class="fieldset-legend px-1">Puppet skin</legend>
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
        Keep current skin
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
      Use default skin
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
      Choose a PNG file
    </label>
    <input
      class="file-input file-input-sm mt-2 w-full"
      type="file"
      accept="image/png"
      disabled={busy}
      onchange={(event) => onselect(event.currentTarget.files?.[0] ?? null)}
    />
    <p class="label">Skins must be exactly 64×64 pixels.</p>
  </fieldset>
</div>
