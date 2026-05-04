<script lang="ts">
	import type { Snippet } from 'svelte';

	let { title, children }: { title: string; children: Snippet } = $props();
</script>

<main class="grid min-h-screen place-items-center bg-[#050507] px-5 py-10 text-[#f2f2f2]">
	<div class="w-full max-w-sm">
		<a
			class="mx-auto mb-6 grid h-14 w-14 place-items-center rounded-full border border-[#3d3a39] bg-[#101010] text-2xl font-light text-[#00d992] drop-shadow-[0_0_8px_rgba(0,217,146,0.35)]"
			href="/"
		>
			R
		</a>
		<h1 class="mb-4 text-center font-[system-ui,sans-serif] text-2xl font-normal tracking-[-0.04em]">
			{title}
		</h1>

		{@render children()}
	</div>
</main>
