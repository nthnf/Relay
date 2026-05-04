<script lang="ts">
	import { EllipsisVertical, MessageCircle, Search, X } from '@lucide/svelte';
	import PrimarySidebar from '$lib/components/chat/PrimarySidebar.svelte';
	import type { ActionData, PageData } from './$types';

	type Tab = 'friends' | 'pending' | 'blocked' | 'add';

	let { data, form }: { data: PageData; form: ActionData } = $props();
	let activeTab = $state<Tab>('friends');
	let search = $state('');

	let filteredFriends = $derived(
		data.friends.filter((friend) => {
			return matchesSearch(friend.profile, friend.friendUserId, friend.routeUserId);
		})
	);
	let filteredIncoming = $derived(
		data.incoming.filter((request) => matchesSearch(request.requester, request.requesterUserId, request.routeUserId))
	);
	let filteredOutgoing = $derived(
		data.outgoing.filter((request) => matchesSearch(request.addressee, request.addresseeUserId, request.routeUserId))
	);
	let filteredBlocked = $derived(
		data.blocked.filter((blocked) => matchesSearch(blocked.profile, blocked.targetUserId, blocked.routeUserId))
	);

	const tabs: { id: Tab; label: string; count?: number }[] = $derived([
		{ id: 'friends', label: 'All', count: data.friends.length },
		{ id: 'pending', label: 'Pending', count: data.incoming.length + data.outgoing.length },
		{ id: 'blocked', label: 'Blocked', count: data.blocked.length },
		{ id: 'add', label: 'Add Friend' }
	]);

	function matchesSearch(profile: { displayName?: string; username?: string } | undefined, ...ids: string[]) {
		const needle = search.trim().toLowerCase();

		if (!needle) {
			return true;
		}

		return `${profile?.displayName ?? ''} ${profile?.username ?? ''} ${ids.join(' ')}`.toLowerCase().includes(needle);
	}
</script>

<svelte:head><title>Friends</title></svelte:head>

<div class="flex min-h-screen bg-abyss text-snow">
	<PrimarySidebar sidebar={data.sidebar} active="friend" />

	<main class="flex min-h-screen min-w-0 flex-1 flex-col bg-chat-main">
		<header class="flex h-12 items-center border-b border-warm-charcoal bg-chat-main px-4">
			<nav class="flex w-full items-center gap-2" aria-label="Friend tabs">
				{#each tabs as tab (tab.id)}
					<button
						class={[
							'rounded-md px-3 py-1.5 text-sm font-semibold transition',
							tab.id === 'add'
								? activeTab === 'add'
									? 'bg-signal text-abyss'
									: 'bg-signal/15 text-mint hover:bg-signal/25'
								: activeTab === tab.id
									? 'bg-warm-charcoal text-snow'
									: 'text-steel hover:bg-warm-charcoal/50 hover:text-snow'
						]}
						type="button"
						onclick={() => (activeTab = tab.id)}
					>
						{tab.label}
						{#if tab.count !== undefined}
							<span class="ml-1 text-xs opacity-70">{tab.count}</span>
						{/if}
					</button>
				{/each}
			</nav>
		</header>

		<section class="flex-1 overflow-y-auto px-5 py-4">
			{#if form?.error}
				<p class="mb-4 rounded-md border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">{form.error}</p>
			{/if}

			{#if activeTab !== 'add'}
				<label class="mb-4 flex w-full items-center gap-3 rounded-md border border-warm-charcoal bg-carbon px-3 py-2 text-steel focus-within:border-signal">
					<Search size={18} strokeWidth={2} />
					<span class="sr-only">Search friends</span>
					<input bind:value={search} class="min-w-0 flex-1 bg-transparent text-sm text-snow outline-none placeholder:text-steel" placeholder="Search" />
				</label>
			{/if}

			{#if activeTab === 'friends'}
				<div class="w-full">
					<p class="mb-3 text-xs font-semibold uppercase tracking-[0.08em] text-parchment">All Friends - {filteredFriends.length}</p>
					{#if filteredFriends.length === 0}
						{@render EmptyState('No friends found', 'Try another search or add a friend.')}
					{:else}
						<div class="divide-y divide-warm-charcoal border-t border-warm-charcoal">
							{#each filteredFriends as friend (friend.friendUserId)}
								<article class="group flex items-center gap-3 px-2 py-3 hover:bg-warm-charcoal/35">
									<span class="relative grid h-10 w-10 shrink-0 place-items-center rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_70%)] text-xs font-bold">
										{(friend.profile?.displayName ?? friend.friendUserId).slice(0, 1).toUpperCase()}
										<i class="absolute right-0 bottom-0 h-3 w-3 rounded-full border-2 border-carbon bg-signal"></i>
									</span>
									<div class="min-w-0 flex-1">
										<p class="truncate text-sm font-semibold text-snow">{friend.profile?.displayName ?? friend.friendUserId}</p>
										<p class="truncate text-xs text-steel">{friend.profile ? `@${friend.profile.username}` : 'Online'}</p>
									</div>
									<div class="flex gap-2 opacity-80 group-hover:opacity-100">
										<form method="POST" action="?/createDm">
											<input type="hidden" name="peerUserId" value={friend.routeUserId} />
											<button class="grid h-8 w-8 place-items-center rounded-full bg-carbon text-parchment hover:text-mint" type="submit" aria-label="Open DM"><MessageCircle size={16} strokeWidth={2.2} /></button>
										</form>
										<form method="POST" action="?/removeFriend">
											<input type="hidden" name="friendUserId" value={friend.routeUserId} />
											<button class="grid h-8 w-8 place-items-center rounded-full bg-carbon text-parchment hover:text-red-200" type="submit" aria-label="Remove friend"><X size={16} strokeWidth={2.2} /></button>
										</form>
										<form method="POST" action="?/blockUser">
											<input type="hidden" name="targetUserId" value={friend.routeUserId} />
											<button class="grid h-8 w-8 place-items-center rounded-full bg-carbon text-parchment hover:text-red-200" type="submit" aria-label="Block user"><EllipsisVertical size={16} strokeWidth={2.2} /></button>
										</form>
									</div>
								</article>
							{/each}
						</div>
					{/if}
				</div>
			{:else if activeTab === 'pending'}
				<div class="grid w-full gap-4 xl:grid-cols-2">
					{@render RequestColumn('Incoming requests', filteredIncoming, 'incoming')}
					{@render RequestColumn('Outgoing requests', filteredOutgoing, 'outgoing')}
				</div>
			{:else if activeTab === 'blocked'}
				<div class="w-full space-y-4">
					{#if filteredBlocked.length === 0}
						{@render EmptyState('No blocked users', search ? 'Try another search.' : 'Blocked users will appear here.')}
					{:else}
						<div class="divide-y divide-warm-charcoal border-t border-warm-charcoal">
							{#each filteredBlocked as blocked (blocked.targetUserId)}
								<article class="flex items-center gap-3 px-2 py-3 hover:bg-warm-charcoal/35">
									<span class="grid h-10 w-10 shrink-0 place-items-center rounded-full border border-warm-charcoal bg-carbon text-xs font-bold">{(blocked.profile?.displayName ?? blocked.targetUserId).slice(0, 1).toUpperCase()}</span>
									<div class="min-w-0 flex-1">
										<p class="truncate text-sm font-semibold">{blocked.profile?.displayName ?? blocked.targetUserId}</p>
										<p class="truncate text-xs text-steel">{blocked.profile ? `@${blocked.profile.username}` : blocked.targetUserId}</p>
									</div>
									<form method="POST" action="?/unblockUser">
										<input type="hidden" name="targetUserId" value={blocked.routeUserId} />
										<button class="rounded-md border border-warm-charcoal px-3 py-2 text-xs font-semibold text-snow hover:border-signal" type="submit">Unblock</button>
									</form>
								</article>
							{/each}
						</div>
					{/if}
				</div>
			{:else}
				<div class="flex min-h-[calc(100vh-8rem)] items-center justify-center py-8">
					<section class="w-full max-w-3xl overflow-hidden rounded-xl border border-warm-charcoal bg-carbon">
						<div class="border-b border-warm-charcoal bg-[radial-gradient(circle_at_82%_0%,var(--color-signal)_0%,transparent_34%)] px-8 py-8">
							<p class="text-xs font-semibold uppercase tracking-[0.12em] text-mint">Create Connection</p>
							<h2 class="mt-3 font-[system-ui,sans-serif] text-3xl font-semibold tracking-[-0.05em] text-snow">Add friend</h2>
							<p class="mt-3 max-w-2xl text-sm leading-6 text-steel">Send a friend request with their Relay handle. Once accepted, they appear in your friends list and direct messages become one click away.</p>
						</div>

						<form method="POST" action="?/requestFriend" class="grid min-h-72 content-center gap-6 px-8 py-10">
							<label class="space-y-2" for="targetUsername">
								<span class="text-sm font-semibold text-parchment">Username</span>
								<input
									id="targetUsername"
									name="targetUsername"
									class="w-full rounded-lg border border-warm-charcoal bg-chat-main px-4 py-4 text-sm text-snow outline-none placeholder:text-steel focus:border-signal"
									placeholder="demo.agent#0420"
								/>
							</label>

							<div class="flex items-center justify-between gap-4 max-sm:flex-col max-sm:items-stretch">
								<button class="rounded-md bg-signal px-5 py-3 text-sm font-semibold text-abyss hover:bg-mint" type="submit">Send Friend Request</button>
							</div>
						</form>
					</section>
				</div>
			{/if}
		</section>
	</main>
</div>

{#snippet EmptyState(title: string, description: string)}
	<div class="mx-auto mt-12 max-w-xl rounded-lg border border-dashed border-dashed-slate/40 bg-carbon p-8 text-center">
		<p class="font-semibold text-snow">{title}</p>
		<p class="mt-2 text-sm text-steel">{description}</p>
	</div>
{/snippet}

{#snippet RequestColumn(title: string, requests: PageData['incoming'], mode: 'incoming' | 'outgoing')}
	<section class="overflow-hidden rounded-lg border border-warm-charcoal bg-carbon">
		<div class="flex items-center justify-between border-b border-warm-charcoal px-4 py-3">
			<h2 class="font-[system-ui,sans-serif] text-lg font-semibold tracking-[-0.03em] text-snow">{title}</h2>
			<span class="rounded-full bg-chat-main px-2 py-1 text-xs font-semibold text-parchment">{requests.length}</span>
		</div>
		{#if requests.length === 0}
			<p class="px-4 py-8 text-sm text-steel">No {mode} requests.</p>
		{:else}
			<div class="divide-y divide-warm-charcoal">
				{#each requests as request (request.friendRequestId)}
					{@render RequestRow(request, mode)}
				{/each}
			</div>
		{/if}
	</section>
{/snippet}

{#snippet RequestRow(request: PageData['incoming'][number], mode: 'incoming' | 'outgoing')}
	{@const targetUserId = mode === 'incoming' ? request.requesterUserId : request.addresseeUserId}
	{@const profile = mode === 'incoming' ? request.requester : request.addressee}
	{@const displayName = profile?.displayName ?? targetUserId}
	{@const username = profile?.username ? `@${profile.username}` : targetUserId}
	<article class="group flex items-center gap-3 px-4 py-3 hover:bg-warm-charcoal/35">
		<span class="grid h-10 w-10 shrink-0 place-items-center overflow-hidden rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_70%)] text-xs font-bold text-snow">
			{#if profile?.avatarUrl}
				<img class="h-full w-full object-cover" src={profile.avatarUrl} alt="" />
			{:else}
				{displayName.slice(0, 1).toUpperCase()}
			{/if}
		</span>
		<div class="min-w-0 flex-1">
			<p class="truncate text-sm font-semibold text-snow">{displayName}</p>
			<p class="truncate text-xs text-steel">{username}</p>
		</div>
		<div class="flex shrink-0 gap-2 opacity-90 group-hover:opacity-100 max-sm:flex-col">
			{#if mode === 'incoming'}
				<form method="POST" action="?/acceptRequest">
					<input type="hidden" name="friendRequestId" value={request.friendRequestId} />
					<button class="rounded-md bg-signal px-3 py-2 text-xs font-semibold text-abyss hover:bg-mint" type="submit">Accept</button>
				</form>
			{/if}
			<form method="POST" action="?/rejectRequest">
				<input type="hidden" name="friendRequestId" value={request.friendRequestId} />
				<button class="rounded-md border border-warm-charcoal px-3 py-2 text-xs font-semibold text-snow hover:border-signal" type="submit">{mode === 'incoming' ? 'Reject' : 'Cancel'}</button>
			</form>
		</div>
	</article>
{/snippet}
