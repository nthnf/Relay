<script lang="ts">
	import { Hash } from '@lucide/svelte';

	let { firstChannelName = 'general' }: { firstChannelName?: string } = $props();
	let channelName = $state('general');

	const presets = ['general', 'design-review', 'resource-log', 'brainstorm'];

	$effect(() => {
		channelName = firstChannelName;
	});
</script>

<section class="flex flex-col rounded-2xl border border-warm-charcoal bg-carbon p-6 md:p-7">
	<div>
		<p class="text-xs font-semibold uppercase tracking-[0.14em] text-mint">First Channel</p>
		<h2 class="mt-3 font-[system-ui,sans-serif] text-xl font-semibold tracking-[-0.04em] text-snow">Shape the first room</h2>
		<p class="mt-4 max-w-xl text-sm leading-6 text-steel">Create the first text channel people land in after joining.</p>
	</div>

	<label class="mt-12 block space-y-3" for="firstChannelName">
		<span class="text-xs font-semibold uppercase tracking-[0.14em] text-parchment">Channel name</span>
		<div class="flex items-center rounded-xl border border-warm-charcoal bg-chat-main px-4 focus-within:border-signal">
			<Hash size={16} strokeWidth={2.2} class="text-steel" />
			<input id="firstChannelName" name="firstChannelName" bind:value={channelName} class="min-w-0 flex-1 bg-transparent px-3 py-4 text-sm text-snow outline-none placeholder:text-steel" placeholder="design-review" />
		</div>
	</label>

	<div class="mt-12">
		<p class="text-xs font-semibold uppercase tracking-[0.14em] text-parchment">Quick presets</p>
		<div class="mt-4 flex flex-wrap gap-2">
			{#each presets as preset (preset)}
				<button class="rounded-full bg-steel/25 px-3 py-1.5 text-xs font-semibold text-parchment hover:bg-signal/15 hover:text-mint" type="button" onclick={() => (channelName = preset)}>{preset}</button>
			{/each}
		</div>
	</div>
</section>
