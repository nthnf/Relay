<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { onMount } from 'svelte';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	onMount(() => {
		const interval = window.setInterval(() => void invalidateAll(), 3_000);
		return () => window.clearInterval(interval);
	});
</script>

<svelte:head><title>#{data.channel.name} - {data.workspace.name}</title></svelte:head>

<main class="mx-auto grid min-h-screen max-w-5xl grid-rows-[auto_1fr_auto] bg-zinc-50 text-zinc-950">
	<header class="border-b border-zinc-200 bg-white px-5 py-4">
		<p class="text-sm font-medium uppercase tracking-[0.2em] text-zinc-500">{data.workspace.name}</p>
		<h1 class="text-2xl font-semibold">#{data.channel.name}</h1>
		<p class="text-sm text-zinc-500">{data.channel.channelKind}</p>
	</header>

	<section class="space-y-3 overflow-y-auto px-5 py-6">
		{#if data.messages.messages.length === 0}
			<p class="rounded-2xl border border-dashed border-zinc-300 bg-white p-6 text-center text-zinc-500">
				No messages yet.
			</p>
		{:else}
			{#each data.messages.messages as message (message.messageId)}
				<article class="rounded-2xl bg-white p-4 shadow-sm ring-1 ring-zinc-200">
					<div class="mb-2 flex items-center justify-between gap-4 text-xs text-zinc-500">
						<span>{message.authorUserId}</span>
						<span>{message.createdAt?.toLocaleString()}</span>
					</div>
					<p class="whitespace-pre-wrap leading-relaxed">{message.deletedAt ? 'Message deleted' : message.body}</p>
				</article>
			{/each}
		{/if}
	</section>

	<form method="POST" action="?/send" class="border-t border-zinc-200 bg-white p-5">
		{#if form?.error}
			<p class="mb-3 rounded-xl bg-red-50 px-4 py-3 text-sm text-red-700">{form.error}</p>
		{/if}

		<label class="sr-only" for="body">Message</label>
		<div class="flex gap-3">
			<textarea
				id="body"
				name="body"
				rows="2"
				class="min-h-14 flex-1 resize-none rounded-2xl border border-zinc-300 bg-zinc-50 px-4 py-3 outline-none placeholder:text-zinc-400 focus:border-zinc-950"
				placeholder="Message #{data.channel.name}"
			>{form?.body ?? ''}</textarea>
			<button class="rounded-2xl bg-zinc-950 px-5 font-semibold text-white hover:bg-zinc-800" type="submit">
				Send
			</button>
		</div>
	</form>
</main>
