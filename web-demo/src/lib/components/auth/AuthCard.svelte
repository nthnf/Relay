<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children, class: className = '' }: { children: Snippet; class?: string } = $props();
</script>

<div class={['rounded-lg border border-[#3d3a39] bg-[#101010] p-5 shadow-[rgba(92,88,85,0.2)_0px_0px_15px]', className]}>
	{@render children()}
</div>
