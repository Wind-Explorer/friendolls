<script lang="ts">
  let {
    kind = "info",
    children,
  }: {
    kind?: "info" | "success" | "error" | "warning";
    children: import("svelte").Snippet;
  } = $props();
</script>

<div
  class:alert-info={kind === "info"}
  class:alert-success={kind === "success"}
  class:alert-error={kind === "error"}
  class:alert-warning={kind === "warning"}
  class="alert alert-soft min-h-0 px-2 py-1.5 text-xs"
  role={kind === "error" ? "alert" : "status"}
>
  {@render children()}
</div>
