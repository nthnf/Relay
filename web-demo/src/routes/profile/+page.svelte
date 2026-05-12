<script lang="ts">
	import { resolve } from '$app/paths';
	import { Edit3, LogOut, MessageCircle, ShieldCheck, Sparkles, UserRound } from '@lucide/svelte';
	import PrimarySidebar from '$lib/components/chat/PrimarySidebar.svelte';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();
	let editing = $state(false);

	const viewer = $derived(data.sidebar?.viewer);
	const displayName = $derived(viewer?.displayName || viewer?.username || 'Relay User');
	const username = $derived(viewer?.username || 'relay.user');
	const initial = $derived(displayName.slice(0, 1).toUpperCase());
</script>

<svelte:head><title>Profile</title></svelte:head>

<div class="flex min-h-screen bg-abyss text-snow">
	<PrimarySidebar sidebar={data.sidebar} />

	<main class="grid min-h-screen min-w-0 flex-1 place-items-center overflow-y-auto bg-chat-main px-5 py-10">
		<section class="w-full max-w-2xl overflow-hidden rounded-2xl border border-warm-charcoal bg-carbon">
			<div class="h-32 border-b border-warm-charcoal bg-[radial-gradient(circle_at_78%_0%,var(--color-signal)_0%,transparent_34%),linear-gradient(135deg,var(--color-carbon),var(--color-abyss))]"></div>

			<div class="px-7 pb-7 text-center">
				<div class="mx-auto -mt-14 grid h-28 w-28 place-items-center rounded-full border-4 border-carbon bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_68%)] font-[system-ui,sans-serif] text-4xl font-bold text-snow">
					{initial}
				</div>

				<h1 class="mt-4 font-[system-ui,sans-serif] text-3xl font-semibold tracking-[-0.05em] text-snow">{displayName}</h1>
				<p class="mt-1 text-sm text-steel">@{username}</p>

				<div class="mx-auto mt-5 flex w-fit items-center gap-2 rounded-full border border-warm-charcoal bg-chat-main px-3 py-1.5 text-xs font-semibold text-mint">
					<span class="h-2 w-2 rounded-full bg-signal"></span>
					Online
				</div>

				<div class="mt-7 grid gap-3 sm:grid-cols-3">
					<div class="rounded-xl border border-warm-charcoal bg-chat-main p-4">
						<UserRound class="mx-auto text-parchment" size={20} strokeWidth={2} />
						<p class="mt-3 text-xs font-semibold uppercase tracking-[0.08em] text-parchment">Handle</p>
						<p class="mt-1 truncate text-sm text-snow">{username}#0420</p>
					</div>
					<div class="rounded-xl border border-warm-charcoal bg-chat-main p-4">
						<MessageCircle class="mx-auto text-parchment" size={20} strokeWidth={2} />
						<p class="mt-3 text-xs font-semibold uppercase tracking-[0.08em] text-parchment">DMs</p>
						<p class="mt-1 text-sm text-snow">Available</p>
					</div>
					<div class="rounded-xl border border-warm-charcoal bg-chat-main p-4">
						<ShieldCheck class="mx-auto text-parchment" size={20} strokeWidth={2} />
						<p class="mt-3 text-xs font-semibold uppercase tracking-[0.08em] text-parchment">Safety</p>
						<p class="mt-1 text-sm text-snow">Protected</p>
					</div>
				</div>

				<div class="mt-7 grid gap-3 sm:grid-cols-2">
					<button class="flex items-center justify-center gap-2 rounded-md border border-warm-charcoal bg-chat-main px-4 py-3 text-sm font-semibold text-snow hover:border-signal" type="button" onclick={() => (editing = !editing)}>
						<Edit3 size={16} strokeWidth={2} />
						Edit profile
					</button>
					<a class="flex items-center justify-center gap-2 rounded-md border border-warm-charcoal bg-chat-main px-4 py-3 text-sm font-semibold text-snow hover:border-signal" href={resolve('/friends')}>
						<Sparkles size={16} strokeWidth={2} />
						Friends
					</a>
				</div>

				{#if editing || form?.error}
					<form method="POST" action="?/update" class="mt-5 rounded-xl border border-warm-charcoal bg-chat-main p-4 text-left">
						{#if form?.error}
							<p class="mb-3 rounded-md border border-red-400/30 bg-red-500/10 px-3 py-2 text-sm text-red-200">{form.error}</p>
						{/if}
						<div class="grid gap-3">
							<label class="space-y-1.5" for="displayName">
								<span class="text-xs font-semibold uppercase tracking-[0.08em] text-parchment">Display name</span>
								<input id="displayName" name="displayName" value={form?.displayName ?? viewer?.displayName ?? ''} class="w-full rounded-md border border-warm-charcoal bg-carbon px-3 py-2.5 text-sm text-snow outline-none placeholder:text-steel focus:border-signal" placeholder="Relay User" />
							</label>
							<label class="space-y-1.5" for="avatarUrl">
								<span class="text-xs font-semibold uppercase tracking-[0.08em] text-parchment">Avatar URL</span>
								<input id="avatarUrl" name="avatarUrl" value={form?.avatarUrl ?? viewer?.avatarUrl ?? ''} class="w-full rounded-md border border-warm-charcoal bg-carbon px-3 py-2.5 text-sm text-snow outline-none placeholder:text-steel focus:border-signal" placeholder="https://..." />
							</label>
						</div>
						<div class="mt-4 flex justify-end gap-2">
							<button class="rounded-md border border-warm-charcoal px-4 py-2 text-sm font-semibold text-snow hover:border-signal" type="button" onclick={() => (editing = false)}>Cancel</button>
							<button class="rounded-md bg-signal px-4 py-2 text-sm font-semibold text-abyss hover:bg-mint" type="submit">Save</button>
						</div>
					</form>
				{/if}

				<form method="POST" action="/auth/logout" class="mt-3">
					<button class="flex w-full items-center justify-center gap-2 rounded-md border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm font-semibold text-red-200 hover:border-red-300/60" type="submit">
						<LogOut size={16} strokeWidth={2} />
						Log out
					</button>
				</form>
			</div>
		</section>
	</main>
</div>
