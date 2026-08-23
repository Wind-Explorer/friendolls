<script lang="ts">
  import { tick } from "svelte";
  import { fly } from "svelte/transition";
  import type { Snippet } from "svelte";

  type Props = {
    id: string;
    label: string;
    labelledBy: string;
    open: boolean;
    onOpenChange: (open: boolean, trigger: HTMLButtonElement) => void;
    trigger: Snippet;
    children: Snippet;
    class?: string;
    triggerClass?: string;
    panelClass?: string;
    panelStyle?: string;
  };

  let {
    id,
    label,
    labelledBy,
    open,
    onOpenChange,
    trigger,
    children,
    class: className = "",
    triggerClass = "",
    panelClass = "",
    panelStyle,
  }: Props = $props();

  let root = $state<HTMLDivElement>();
  let panel = $state<HTMLDivElement>();
  let triggerButton = $state<HTMLButtonElement>();

  $effect(() => {
    if (open) {
      tick().then(() => {
        if (open) panel?.focus({ preventScroll: true });
      });
    }
  });

  function setOpen(nextOpen: boolean) {
    if (triggerButton) onOpenChange(nextOpen, triggerButton);
  }

  function closeOnEscape(event: KeyboardEvent) {
    if (open && event.key === "Escape") {
      setOpen(false);
      triggerButton?.focus({ preventScroll: true });
    }
  }

  function closeOnOutsidePointer(event: PointerEvent) {
    if (
      open &&
      root &&
      event.target instanceof Node &&
      !root.contains(event.target)
    ) {
      setOpen(false);
    }
  }

  function closeOnFocusOut(event: FocusEvent) {
    if (
      open &&
      root &&
      (!(event.relatedTarget instanceof Node) ||
        !root.contains(event.relatedTarget))
    ) {
      setOpen(false);
    }
  }

  function closeOnWindowBlur() {
    if (open) setOpen(false);
  }
</script>

<svelte:window
  onkeydown={closeOnEscape}
  onpointerdown={closeOnOutsidePointer}
  onblur={closeOnWindowBlur}
/>

<div
  bind:this={root}
  class={`relative grid place-items-center ${className}`}
  onfocusout={closeOnFocusOut}
>
  {#if open}
    <div
      bind:this={panel}
      {id}
      class={`absolute z-20 focus:outline-none ${panelClass}`}
      style={panelStyle}
      role="dialog"
      aria-labelledby={labelledBy}
      tabindex="-1"
    >
      <div transition:fly={{ y: 12, duration: 180 }}>
        {@render children()}
      </div>
    </div>
  {/if}

  <button
    bind:this={triggerButton}
    type="button"
    class={`grid cursor-pointer place-items-center border-0 bg-transparent p-0 transition-transform duration-150 hover:-translate-y-0.5 focus-visible:-translate-y-0.5 ${triggerClass}`}
    aria-label={label}
    aria-expanded={open}
    aria-controls={id}
    onclick={() => setOpen(!open)}
  >
    {@render trigger()}
  </button>
</div>
