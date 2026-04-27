<script lang="ts">
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();
</script>

<svelte:head><title>Friends</title></svelte:head>

<main class="mx-auto min-h-screen max-w-4xl px-5 py-8 text-slate-100">
	<header class="mb-8">
		<p class="text-sm uppercase tracking-[0.2em] text-cyan-300">People</p>
		<h1 class="text-3xl font-semibold">Friends</h1>
	</header>

	{#if form?.error}
		<p class="mb-5 rounded-xl bg-red-500/15 px-4 py-3 text-sm text-red-200">{form.error}</p>
	{/if}

	<section class="mb-8 rounded-3xl bg-white/5 p-5 ring-1 ring-white/10">
		<h2 class="mb-4 text-lg font-semibold">Send friend request</h2>
		<form method="POST" action="?/requestFriend" class="flex gap-3 max-sm:flex-col">
			<label class="sr-only" for="targetUserId">Target user ID</label>
			<input
				id="targetUserId"
				name="targetUserId"
				class="flex-1 rounded-2xl border border-white/10 bg-white/10 px-4 py-3 outline-none placeholder:text-slate-500 focus:border-cyan-300"
				placeholder="Target user UUID"
			/>
			<button class="rounded-2xl bg-cyan-300 px-5 font-semibold text-slate-950 hover:bg-cyan-200" type="submit">
				Request
			</button>
		</form>
	</section>

	<section class="space-y-3">
		<h2 class="text-lg font-semibold">Friend list</h2>
		{#if data.friends.length === 0}
			<p class="rounded-2xl border border-dashed border-white/15 p-6 text-center text-slate-400">
				No friends yet.
			</p>
		{:else}
			{#each data.friends as friend (friend.friendUserId)}
				<article class="grid gap-4 rounded-2xl bg-white/5 p-4 ring-1 ring-white/10 sm:grid-cols-[1fr_auto]">
					<div>
						<p class="font-semibold">{friend.profile?.displayName ?? friend.friendUserId}</p>
						<p class="text-sm text-slate-400">{friend.profile ? `@${friend.profile.username}` : friend.friendUserId}</p>
					</div>
					<div class="flex flex-wrap gap-2">
						<form method="POST" action="?/createDm">
							<input type="hidden" name="peerUserId" value={friend.routeUserId} />
							<button class="rounded-xl bg-white px-4 py-2 text-sm font-semibold text-slate-950" type="submit">
								Open DM
							</button>
						</form>
						<form method="POST" action="?/removeFriend">
							<input type="hidden" name="friendUserId" value={friend.routeUserId} />
							<button class="rounded-xl bg-white/10 px-4 py-2 text-sm font-semibold text-slate-100" type="submit">
								Remove
							</button>
						</form>
						<form method="POST" action="?/blockUser">
							<input type="hidden" name="targetUserId" value={friend.routeUserId} />
							<button class="rounded-xl bg-red-500/20 px-4 py-2 text-sm font-semibold text-red-100" type="submit">
								Block
							</button>
						</form>
					</div>
				</article>
			{/each}
		{/if}
	</section>

	<section class="mt-8 grid gap-4 md:grid-cols-2">
		<div class="rounded-3xl bg-white/5 p-5 ring-1 ring-white/10">
			<h2 class="mb-3 text-lg font-semibold">Incoming requests</h2>
			{#if data.incoming.length === 0}
				<p class="text-sm text-slate-400">No incoming requests.</p>
			{:else}
				<div class="space-y-3">
					{#each data.incoming as request (request.friendRequestId)}
						<div class="rounded-2xl bg-white/5 p-3">
							<p class="break-all text-sm text-slate-300">{request.requesterUserId}</p>
							<div class="mt-3 flex gap-2">
								<form method="POST" action="?/acceptRequest">
									<input type="hidden" name="friendRequestId" value={request.friendRequestId} />
									<button class="rounded-xl bg-cyan-300 px-3 py-2 text-sm font-semibold text-slate-950" type="submit">
										Accept
									</button>
								</form>
								<form method="POST" action="?/rejectRequest">
									<input type="hidden" name="friendRequestId" value={request.friendRequestId} />
									<button class="rounded-xl bg-white/10 px-3 py-2 text-sm font-semibold text-slate-100" type="submit">
										Reject
									</button>
								</form>
							</div>
						</div>
					{/each}
				</div>
			{/if}
		</div>
		<div class="rounded-3xl bg-white/5 p-5 ring-1 ring-white/10">
			<h2 class="mb-3 text-lg font-semibold">Outgoing requests</h2>
			{#if data.outgoing.length === 0}
				<p class="text-sm text-slate-400">No outgoing requests.</p>
			{:else}
				<div class="space-y-3">
					{#each data.outgoing as request (request.friendRequestId)}
						<div class="rounded-2xl bg-white/5 p-3">
							<p class="break-all text-sm text-slate-300">{request.addresseeUserId}</p>
							<form method="POST" action="?/rejectRequest" class="mt-3">
								<input type="hidden" name="friendRequestId" value={request.friendRequestId} />
								<button class="rounded-xl bg-white/10 px-3 py-2 text-sm font-semibold text-slate-100" type="submit">
									Cancel
								</button>
							</form>
						</div>
					{/each}
				</div>
			{/if}
		</div>
	</section>

	<section class="mt-8 rounded-3xl bg-white/5 p-5 ring-1 ring-white/10">
		<h2 class="mb-4 text-lg font-semibold">Unblock user</h2>
		<form method="POST" action="?/unblockUser" class="flex gap-3 max-sm:flex-col">
			<label class="sr-only" for="unblockUserId">Target user ID</label>
			<input
				id="unblockUserId"
				name="targetUserId"
				class="flex-1 rounded-2xl border border-white/10 bg-white/10 px-4 py-3 outline-none placeholder:text-slate-500 focus:border-cyan-300"
				placeholder="Target user UUID"
			/>
			<button class="rounded-2xl bg-white px-5 font-semibold text-slate-950" type="submit">Unblock</button>
		</form>
	</section>
</main>
