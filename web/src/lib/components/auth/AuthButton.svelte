<script lang="ts">
	import type { Snippet } from 'svelte';

	let { children, variant = 'primary' }: { children: Snippet; variant?: 'primary' | 'secondary' } = $props();
</script>

<button
	class={[
		'w-full rounded-md px-4 py-2.5 text-sm font-semibold hover:bg-black/20 disabled:opacity-60',
		variant === 'primary'
			? 'bg-[#101010] text-[#2fd6a1] outline outline-1 outline-[#2fd6a1]'
			: 'border border-[#3d3a39] text-[#f2f2f2] hover:border-[#00d992]'
	]}
	type="submit"
>
	{@render children()}
</button>
