<script lang="ts">
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head><title>Relay</title></svelte:head>

<main class="mx-auto min-h-screen max-w-5xl px-5 py-8 text-slate-100">
	<header class="mb-8">
		<p class="text-sm uppercase tracking-[0.2em] text-cyan-300">Bootstrap</p>
		<h1 class="text-3xl font-semibold">Welcome {data.viewer?.displayName ?? 'to Relay'}</h1>
		<p class="text-sm text-slate-400">{data.pendingFriendRequestCount} pending friend requests</p>
	</header>

	{#if form?.error}
		<p class="mb-5 rounded-xl bg-red-500/15 px-4 py-3 text-sm text-red-200">{form.error}</p>
	{/if}

	<section class="mb-8 rounded-3xl bg-white/5 p-5 ring-1 ring-white/10">
		<h2 class="mb-4 text-lg font-semibold">Create workspace</h2>
		<form method="POST" action="?/createWorkspace" class="grid gap-3 sm:grid-cols-[1fr_1fr_auto]">
			<label class="sr-only" for="name">Workspace name</label>
			<input
				id="name"
				name="name"
				class="rounded-2xl border border-white/10 bg-white/10 px-4 py-3 outline-none placeholder:text-slate-500 focus:border-cyan-300"
				placeholder="Workspace name"
			/>
			<label class="sr-only" for="firstChannelName">First channel</label>
			<input
				id="firstChannelName"
				name="firstChannelName"
				class="rounded-2xl border border-white/10 bg-white/10 px-4 py-3 outline-none placeholder:text-slate-500 focus:border-cyan-300"
				placeholder="general"
			/>
			<button class="rounded-2xl bg-cyan-300 px-5 font-semibold text-slate-950 hover:bg-cyan-200" type="submit">
				Create
			</button>
		</form>
	</section>

	<section class="grid gap-4 md:grid-cols-2">
		{#if data.workspaces.length === 0}
			<p class="rounded-2xl border border-dashed border-white/15 p-6 text-center text-slate-400 md:col-span-2">
				No workspaces yet.
			</p>
		{:else}
			{#each data.workspaces as workspace (workspace.workspaceId)}
				<a class="rounded-3xl bg-white/5 p-5 ring-1 ring-white/10 hover:bg-white/10" href={`/workspace/${workspace.routeId}`}>
					<p class="text-lg font-semibold">{workspace.name}</p>
					<p class="text-sm text-slate-400">{workspace.unreadCount} unread</p>
				</a>
			{/each}
		{/if}
	</section>
</main>
