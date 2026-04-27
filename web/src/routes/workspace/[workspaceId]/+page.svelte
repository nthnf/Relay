<script lang="ts">
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head><title>{data.workspace.name}</title></svelte:head>

<main class="mx-auto min-h-screen max-w-4xl px-5 py-8 text-slate-100">
	<header class="mb-8">
		<p class="text-sm uppercase tracking-[0.2em] text-cyan-300">Workspace</p>
		<h1 class="text-3xl font-semibold">{data.workspace.name}</h1>
		<p class="text-sm text-slate-400">{data.workspace.memberCount} members</p>
	</header>

	{#if form?.error}
		<p class="mb-5 rounded-xl bg-red-500/15 px-4 py-3 text-sm text-red-200">{form.error}</p>
	{/if}

	<section class="mb-8 rounded-3xl bg-white/5 p-5 ring-1 ring-white/10">
		<h2 class="mb-4 text-lg font-semibold">Create channel</h2>
		<form method="POST" action="?/createChannel" class="grid gap-3 sm:grid-cols-[1fr_auto_auto]">
			<label class="sr-only" for="name">Channel name</label>
			<input
				id="name"
				name="name"
				class="rounded-2xl border border-white/10 bg-white/10 px-4 py-3 outline-none placeholder:text-slate-500 focus:border-cyan-300"
				placeholder="general"
			/>
			<select
				name="channelKind"
				class="rounded-2xl border border-white/10 bg-white/10 px-4 py-3 outline-none focus:border-cyan-300"
			>
				<option value="text">text</option>
			</select>
			<button class="rounded-2xl bg-cyan-300 px-5 font-semibold text-slate-950 hover:bg-cyan-200" type="submit">
				Create
			</button>
		</form>
	</section>

	<section class="space-y-3">
		<h2 class="text-lg font-semibold">Channels</h2>
		{#if data.channels.length === 0}
			<p class="rounded-2xl border border-dashed border-white/15 p-6 text-center text-slate-400">
				No projected channels yet.
			</p>
		{:else}
			{#each data.channels as channel (channel.channelId)}
				<a
					class="block rounded-2xl bg-white/5 p-4 ring-1 ring-white/10 hover:bg-white/10"
					href={`/workspace/${data.workspaceRouteId}/${channel.routeId}`}
				>
					<p class="font-semibold">#{channel.name}</p>
					<p class="text-sm text-slate-400">{channel.channelKind}</p>
				</a>
			{/each}
		{/if}
	</section>
</main>
