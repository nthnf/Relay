<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<aside class="hidden h-screen w-72 shrink-0 overflow-y-auto border-r border-warm-charcoal bg-carbon p-5 md:block">
	{@render children()}
</aside>
