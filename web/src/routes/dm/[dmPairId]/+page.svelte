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

<svelte:head><title>DM with {data.thread.peerDisplayName}</title></svelte:head>

<main class="mx-auto flex min-h-screen max-w-3xl flex-col bg-slate-950 text-slate-100">
	<header class="border-b border-white/10 px-5 py-4">
		<p class="text-sm uppercase tracking-[0.2em] text-cyan-300">Direct message</p>
		<h1 class="text-2xl font-semibold">{data.thread.peerDisplayName}</h1>
		<p class="text-sm text-slate-400">@{data.thread.peerUsername}</p>
	</header>

	<section class="flex-1 space-y-3 overflow-y-auto px-5 py-6">
		{#if data.messages.messages.length === 0}
			<p class="rounded-2xl border border-dashed border-white/15 p-6 text-center text-slate-400">
				No messages yet.
			</p>
		{:else}
			{#each data.messages.messages as message (message.messageId)}
				<article class="rounded-2xl bg-white/5 p-4 shadow-sm ring-1 ring-white/10">
					<div class="mb-2 flex items-center justify-between gap-4 text-xs text-slate-400">
						<span>{message.authorUserId}</span>
						<span>{message.createdAt?.toLocaleString()}</span>
					</div>
					<p class="whitespace-pre-wrap leading-relaxed">{message.deletedAt ? 'Message deleted' : message.body}</p>
				</article>
			{/each}
		{/if}
	</section>

	<form method="POST" action="?/send" class="border-t border-white/10 p-5">
		{#if form?.error}
			<p class="mb-3 rounded-xl bg-red-500/15 px-4 py-3 text-sm text-red-200">{form.error}</p>
		{/if}

		<label class="sr-only" for="body">Message</label>
		<div class="flex gap-3">
			<textarea
				id="body"
				name="body"
				rows="2"
				class="min-h-14 flex-1 resize-none rounded-2xl border border-white/10 bg-white/10 px-4 py-3 text-slate-100 outline-none placeholder:text-slate-500 focus:border-cyan-300"
				placeholder="Write a message"
			>{form?.body ?? ''}</textarea>
			<button class="rounded-2xl bg-cyan-300 px-5 font-semibold text-slate-950 hover:bg-cyan-200" type="submit">
				Send
			</button>
		</div>
	</form>
</main>
