<script lang="ts">
	import { Globe2, ImagePlus, LockKeyhole } from '@lucide/svelte';

	let { error, name = '' }: { error?: string; name?: string } = $props();
	let visibility = $state<'public' | 'private'>('public');
</script>

<section class="rounded-2xl border border-warm-charcoal bg-carbon p-6 md:p-7">
	<p class="text-xs font-semibold uppercase tracking-[0.14em] text-mint">Build your Server</p>
	<h1 class="mt-2 font-[system-ui,sans-serif] text-2xl font-semibold tracking-[-0.05em] text-snow">Create the home base for your community.</h1>
	<p class="mt-2 text-sm text-steel">Your server starts here. Give it a name, an identity, and a first channel to gather around.</p>

	{#if error}
		<p class="mt-5 rounded-md border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">{error}</p>
	{/if}

	<div class="mt-8 flex items-end gap-4">
		<button class="grid h-24 w-24 shrink-0 place-items-center rounded-2xl border border-warm-charcoal bg-chat-main text-steel hover:border-signal hover:text-mint" type="button" aria-label="Upload server icon">
			<ImagePlus size={38} strokeWidth={1.8} />
		</button>

		<label class="min-w-0 flex-1 space-y-2" for="name">
			<span class="text-xs font-semibold uppercase tracking-[0.14em] text-parchment">Server brand</span>
			<input id="name" name="name" value={name} class="w-full rounded-xl border border-warm-charcoal bg-chat-main px-4 py-4 text-sm font-semibold text-snow outline-none placeholder:text-steel focus:border-signal" placeholder="Creative Collective" />
		</label>
	</div>

	<label class="mt-6 block space-y-2" for="description">
		<span class="text-xs font-semibold uppercase tracking-[0.14em] text-parchment">Description</span>
		<textarea id="description" name="description" rows="4" class="w-full resize-none rounded-xl border border-warm-charcoal bg-chat-main px-4 py-4 text-sm text-snow outline-none placeholder:text-steel focus:border-signal" placeholder="Share what this server is for, who belongs here, and what people should expect."></textarea>
	</label>

	<div class="mt-8">
		<p class="text-xs font-semibold uppercase tracking-[0.14em] text-parchment">Server visibility</p>
		<div class="mt-3 grid gap-3 md:grid-cols-2">
			<label class={['flex min-h-20 cursor-pointer items-center gap-4 rounded-xl bg-chat-main px-4 py-3 transition', visibility === 'public' ? 'border-2 border-steel text-snow' : 'border border-warm-charcoal text-parchment hover:border-steel']}>
				<input bind:group={visibility} class="sr-only" type="radio" name="visibility" value="public" />
				<span class={['grid h-10 w-10 place-items-center rounded-xl bg-carbon', visibility === 'public' ? 'text-parchment' : 'text-steel']}><Globe2 size={18} strokeWidth={2.2} /></span>
				<span><strong class="block text-sm">Public</strong><small class="mt-1 block text-xs leading-5 text-steel">Open for people to browse and join the community.</small></span>
			</label>

			<label class={['flex min-h-20 cursor-pointer items-center gap-4 rounded-xl bg-chat-main px-4 py-3 transition', visibility === 'private' ? 'border-2 border-steel text-snow' : 'border border-warm-charcoal text-parchment hover:border-steel']}>
				<input bind:group={visibility} class="sr-only" type="radio" name="visibility" value="private" />
				<span class={['grid h-10 w-10 place-items-center rounded-xl bg-carbon', visibility === 'private' ? 'text-parchment' : 'text-steel']}><LockKeyhole size={18} strokeWidth={2.2} /></span>
				<span><strong class="block text-sm text-snow">Private</strong><small class="mt-1 block text-xs leading-5 text-steel">Invite-only with stronger access controls for launch.</small></span>
			</label>
		</div>
	</div>
</section>
