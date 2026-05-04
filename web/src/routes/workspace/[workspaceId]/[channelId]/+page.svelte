<script lang="ts">
	import { goto, invalidateAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { env } from '$env/dynamic/public';
	import ChatHeader from '$lib/components/chat/ChatHeader.svelte';
	import ChatMessageList from '$lib/components/chat/ChatMessageList.svelte';
	import MessageComposer from '$lib/components/chat/MessageComposer.svelte';
	import PrimarySidebar from '$lib/components/chat/PrimarySidebar.svelte';
	import WorkspaceSidebar from '$lib/components/chat/WorkspaceSidebar.svelte';
	import { onMount } from 'svelte';
	import type { ActionData, PageData } from './$types';

	type MemberProfile = {
		userId: string;
		username?: string;
		displayName?: string;
		avatarUrl?: string;
	};

	type FriendProfile = {
		userId: string;
		username?: string;
		displayName?: string;
		avatarUrl?: string;
	};

	type FriendItem = {
		friendUserId: string;
		profile?: FriendProfile;
	};

	type FriendsResponse = {
		friends?: { friendUserId: string }[];
		profiles?: FriendProfile[];
	};

	type InviteLinkResponse = {
		code?: string;
	};
	type MembersResponse = {
		members?: WorkspaceMemberSummary[];
	};
	type WorkspaceMemberSummary = MemberProfile & {
		profile?: MemberProfile;
	};
	type CreateChannelResponse = {
		projected?: { routeId?: string; channelId?: string };
	};

	type PresenceResponse = {
		users?: { userId: string; online: boolean; lastSeenAt?: Date }[];
	};

	let { data, form }: { data: PageData; form: ActionData } = $props();
	let showMembers = $state(false);
	let showInviteModal = $state(false);
	let showCreateChannelModal = $state(false);
	let newChannelName = $state('');
	let createChannelBusy = $state(false);
	let createChannelError = $state('');
	let friends = $state.raw<FriendItem[]>([]);
	let members = $state.raw<WorkspaceMemberSummary[]>([]);
	let membersLoading = $state(false);
	let membersError = $state('');
	let removingMemberId = $state('');
	let friendsLoading = $state(false);
	let inviteBusy = $state(false);
	let inviteMessage = $state('');
	let inviteError = $state('');
	let invitePath = $state('');
	let presenceById = $state.raw<Record<string, boolean>>({});

	let authorNames = $derived(
		Object.fromEntries(data.authorProfiles.map((profile) => [profile.userId, profile.displayName || profile.username || profile.userId]))
	);
	let messages = $derived(
		data.messages.messages.map((message) => ({
			...message,
			senderName: authorNames[message.authorUserId] ?? 'Unknown user'
		}))
	);
	let lastReadSeq = $derived(data.messages.messages.at(-1)?.conversationMessageSeq);

	async function openInviteModal() {
		showInviteModal = true;
		inviteMessage = '';
		inviteError = '';
		invitePath = '';
		await loadFriends();
	}

	async function loadFriends() {
		friendsLoading = true;
		inviteError = '';

		try {
			const response = await fetch('/api/friends?pageSize=100');

			if (!response.ok) {
				throw new Error(await response.text());
			}

			const body = (await response.json()) as FriendsResponse;
			const profilesById = Object.fromEntries((body.profiles ?? []).map((profile) => [profile.userId, profile]));
			friends = (body.friends ?? []).map((friend) => ({
				...friend,
				profile: profilesById[friend.friendUserId]
			}));
		} catch (error) {
			inviteError = error instanceof Error ? error.message : 'Could not load friends.';
			friends = [];
		} finally {
			friendsLoading = false;
		}
	}

	async function addMember(friend: FriendItem) {
		inviteBusy = true;
		inviteMessage = '';
		inviteError = '';

		try {
			const response = await fetch(`/api/workspaces/${data.workspaceRouteId}/members`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ targetUserId: friend.friendUserId })
			});

			if (!response.ok) {
				throw new Error(await response.text());
			}

			inviteMessage = `${displayName(friendProfile(friend))} added to the workspace.`;
			if (showMembers) {
				await loadMembers();
			}
		} catch (error) {
			inviteError = error instanceof Error ? error.message : 'Could not add member.';
		} finally {
			inviteBusy = false;
		}
	}

	async function createInviteLink() {
		inviteBusy = true;
		inviteMessage = '';
		inviteError = '';

		try {
			const response = await fetch(`/api/workspaces/${data.workspaceRouteId}/invite-link`, { method: 'POST' });

			if (!response.ok) {
				throw new Error(await response.text());
			}

			const body = (await response.json()) as InviteLinkResponse;
			invitePath = body.code ? `/invite/${body.code}` : 'Invite link created.';
		} catch (error) {
			inviteError = error instanceof Error ? error.message : 'Could not create invite link.';
		} finally {
			inviteBusy = false;
		}
	}

	async function toggleMembers() {
		showMembers = !showMembers;

		if (showMembers) {
			await loadMembers();
		}
	}

	async function loadMembers() {
		membersLoading = true;
		membersError = '';

		try {
			const response = await fetch(`/api/workspaces/${data.workspaceRouteId}/members?pageSize=200`);

			if (!response.ok) {
				throw new Error(await response.text());
			}

			const body = (await response.json()) as MembersResponse;
			members = (body.members ?? []).sort((a, b) => displayName(memberProfile(a)).localeCompare(displayName(memberProfile(b))));
			await loadPresence();
		} catch (error) {
			membersError = error instanceof Error ? error.message : 'Could not load members.';
			members = [];
			presenceById = {};
		} finally {
			membersLoading = false;
		}
	}

	async function removeMember(member: WorkspaceMemberSummary) {
		const targetUserId = memberProfile(member).userId;

		if (!targetUserId) {
			return;
		}

		removingMemberId = targetUserId;
		membersError = '';

		try {
			const response = await fetch(`/api/workspaces/${data.workspaceRouteId}/members`, {
				method: 'DELETE',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ targetUserId })
			});

			if (!response.ok) {
				throw new Error(await response.text());
			}

			await loadMembers();
		} catch (error) {
			membersError = error instanceof Error ? error.message : 'Could not remove member.';
		} finally {
			removingMemberId = '';
		}
	}

	async function loadPresence() {
		const userIds = members.map((member) => memberProfile(member).userId).filter(Boolean);

		if (userIds.length === 0) {
			presenceById = {};
			return;
		}

		try {
			const params = new URLSearchParams();
			for (const userId of userIds) {
				params.append('userId', userId);
			}

			const response = await fetch(`/api/presence?${params}`);

			if (!response.ok) {
				throw new Error(await response.text());
			}

			const body = (await response.json()) as PresenceResponse;
			presenceById = Object.fromEntries((body.users ?? []).map((presence) => [presence.userId, presence.online]));
		} catch {
			presenceById = {};
		}
	}

	function displayName(profile: { userId?: string; username?: string; displayName?: string }) {
		return profile.displayName || profile.username || profile.userId || 'Unknown user';
	}

	function username(profile: { userId?: string; username?: string }) {
		return profile.username ? `@${profile.username}` : (profile.userId ?? 'Unknown user');
	}

	function friendProfile(friend: FriendItem): FriendProfile {
		return friend.profile ?? { userId: friend.friendUserId };
	}

	function memberProfile(member: WorkspaceMemberSummary): MemberProfile {
		return member.profile ?? member;
	}

	async function createChannel() {
		const name = newChannelName.trim();

		if (!name) {
			createChannelError = 'Channel name is required.';
			return;
		}

		createChannelBusy = true;
		createChannelError = '';

		try {
			const response = await fetch(`/api/workspaces/${data.workspaceRouteId}/channels`, {
				method: 'POST',
				headers: { 'content-type': 'application/json' },
				body: JSON.stringify({ name, channelKind: 'text' })
			});

			if (!response.ok) {
				throw new Error(await response.text());
			}

			const body = (await response.json()) as CreateChannelResponse;
			const routeId = body.projected?.routeId ?? body.projected?.channelId;

			if (routeId) {
				await goto(resolve(`/workspace/${data.workspaceRouteId}/${routeId}`));
			} else {
				await invalidateAll();
				showCreateChannelModal = false;
			}
		} catch (error) {
			createChannelError = error instanceof Error ? error.message : 'Could not create channel.';
		} finally {
			createChannelBusy = false;
		}
	}

	function markConversationRead() {
		if (!data.channel.conversationId || lastReadSeq === undefined) {
			return;
		}

		void fetch(`/api/conversations/${data.channel.conversationId}/read`, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({ lastReadConversationMessageSeq: Number(lastReadSeq) })
		}).catch(() => undefined);
	}

	onMount(() => {
		markConversationRead();

		const viewerUserId = data.sidebar?.viewer?.userId;
		const targetId = data.channel.channelId;

		if (!viewerUserId || !targetId) {
			return;
		}

		const socket = new WebSocket(`${env.PUBLIC_REALTIME_WS_URL || 'ws://localhost:30080/ws'}?user_id=${encodeURIComponent(viewerUserId)}`);
		socket.addEventListener('open', () => {
			socket.send(JSON.stringify({ type: 'subscribe', target_kind: 'workspace_channel', target_id: targetId }));
		});
		socket.addEventListener('message', () => void invalidateAll());
		socket.addEventListener('error', () => undefined);

		return () => socket.close();
	});
</script>

<svelte:head><title>#{data.channel.name} - {data.workspace.name}</title></svelte:head>

<div class="flex min-h-screen bg-abyss text-snow">
	<PrimarySidebar sidebar={data.sidebar} active="workspace" activeWorkspaceId={data.workspace.workspaceId} />
	<WorkspaceSidebar workspace={data.workspace} workspaceRouteId={data.workspaceRouteId} channels={data.channels} activeChannelId={data.channel.channelId} onAddMember={openInviteModal} onCreateChannel={() => (showCreateChannelModal = true)} />

	<main class="grid min-h-screen min-w-0 flex-1 grid-rows-[auto_1fr_auto] bg-chat-main">
		<ChatHeader title={`# ${data.channel.name}`} kicker={data.workspace.name} variant="channel" onToggleMembers={toggleMembers} />
		<ChatMessageList {messages} variant="channel" currentUserId={data.sidebar?.viewer?.userId} />
		<MessageComposer error={form?.error} body={form?.body ?? ''} />
	</main>

	{#if showMembers}
		<aside class="hidden h-screen w-72 shrink-0 border-l border-warm-charcoal bg-carbon lg:flex lg:flex-col">
			<div class="border-b border-warm-charcoal px-4 py-4">
				<p class="text-[0.65rem] font-semibold uppercase tracking-[0.16em] text-signal">Workspace</p>
				<h2 class="mt-1 font-[system-ui,sans-serif] text-lg font-semibold tracking-[-0.04em] text-snow">Members</h2>
				<p class="mt-1 text-xs text-steel">Loaded from workspace membership</p>
			</div>
			<div class="min-h-0 flex-1 overflow-y-auto p-3">
				{#if membersLoading}
					<p class="rounded-md border border-warm-charcoal px-4 py-6 text-sm text-steel">Loading members...</p>
				{:else if membersError}
					<p class="rounded-md border border-red-400/30 bg-red-500/10 px-4 py-3 text-sm text-red-200">{membersError}</p>
				{:else if members.length === 0}
					<p class="rounded-md border border-dashed border-dashed-slate/40 p-4 text-sm text-steel">No members found.</p>
				{:else}
					{#each members as member (memberProfile(member).userId)}
						{@render MemberRow(member)}
					{/each}
				{/if}
			</div>
		</aside>
	{/if}
</div>

{#if showCreateChannelModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center px-4 py-6">
		<button class="absolute inset-0 bg-abyss/75 backdrop-blur-sm" type="button" aria-label="Close create channel dialog" onclick={() => (showCreateChannelModal = false)}></button>
		<form class="relative w-full max-w-md rounded-xl border border-warm-charcoal bg-carbon p-5 shadow-2xl" onsubmit={(event) => { event.preventDefault(); void createChannel(); }}>
			<p class="text-[0.65rem] font-semibold uppercase tracking-[0.16em] text-signal">{data.workspace.name}</p>
			<h2 class="mt-1 font-[system-ui,sans-serif] text-xl font-semibold tracking-[-0.04em] text-snow">Create text channel</h2>
			{#if createChannelError}
				<p class="mt-4 rounded-md border border-red-400/30 bg-red-500/10 px-3 py-2 text-sm text-red-200">{createChannelError}</p>
			{/if}
			<label class="mt-4 block space-y-2" for="channelName">
				<span class="text-sm font-semibold text-parchment">Channel name</span>
				<input id="channelName" bind:value={newChannelName} class="w-full rounded-md border border-warm-charcoal bg-chat-main px-3 py-2.5 text-sm text-snow outline-none placeholder:text-steel focus:border-signal" placeholder="general" />
			</label>
			<div class="mt-5 flex justify-end gap-2">
				<button class="rounded-md border border-warm-charcoal px-4 py-2 text-sm font-semibold text-snow hover:border-signal" type="button" onclick={() => (showCreateChannelModal = false)}>Cancel</button>
				<button class="rounded-md bg-signal px-4 py-2 text-sm font-semibold text-abyss hover:bg-mint disabled:cursor-not-allowed disabled:opacity-60" type="submit" disabled={createChannelBusy}>{createChannelBusy ? 'Creating...' : 'Create'}</button>
			</div>
		</form>
	</div>
{/if}

{#if showInviteModal}
	<div class="fixed inset-0 z-50 flex items-center justify-center px-4 py-6">
		<button class="absolute inset-0 bg-abyss/75 backdrop-blur-sm" type="button" aria-label="Close add member dialog" onclick={() => (showInviteModal = false)}></button>
		<div class="relative max-h-[90vh] w-full max-w-xl overflow-hidden rounded-xl border border-warm-charcoal bg-carbon shadow-2xl" role="dialog" aria-modal="true" aria-labelledby="invite-title">
			<div class="flex items-start justify-between gap-4 border-b border-warm-charcoal px-5 py-4">
				<div>
					<p class="text-[0.65rem] font-semibold uppercase tracking-[0.16em] text-signal">{data.workspace.name}</p>
					<h2 id="invite-title" class="mt-1 font-[system-ui,sans-serif] text-xl font-semibold tracking-[-0.04em] text-snow">Add workspace member</h2>
				</div>
				<button class="rounded-md px-2 py-1 text-sm font-semibold text-parchment hover:bg-warm-charcoal/60 hover:text-snow" type="button" onclick={() => (showInviteModal = false)}>Close</button>
			</div>

			<div class="max-h-[68vh] overflow-y-auto px-5 py-4">
				{#if inviteError}
					<p class="mb-3 rounded-md border border-red-400/30 bg-red-500/10 px-3 py-2 text-sm text-red-200">{inviteError}</p>
				{/if}
				{#if inviteMessage}
					<p class="mb-3 rounded-md border border-signal/30 bg-signal/10 px-3 py-2 text-sm text-mint">{inviteMessage}</p>
				{/if}
				{#if invitePath}
					<p class="mb-3 rounded-md border border-warm-charcoal bg-chat-main px-3 py-2 text-sm text-snow">Invite: <span class="font-mono text-mint">{invitePath}</span></p>
				{/if}

				<p class="mb-3 text-xs font-semibold uppercase tracking-[0.08em] text-parchment">Friends</p>
				{#if friendsLoading}
					<p class="rounded-md border border-warm-charcoal px-4 py-6 text-sm text-steel">Loading friends...</p>
				{:else if friends.length === 0}
					<p class="rounded-md border border-dashed border-dashed-slate/40 px-4 py-6 text-sm text-steel">No friends found.</p>
				{:else}
					<div class="divide-y divide-warm-charcoal border-y border-warm-charcoal">
						{#each friends as friend (friend.friendUserId)}
							{@const profile = friendProfile(friend)}
							<article class="flex items-center gap-3 px-1 py-3">
								{@render Avatar(profile)}
								<div class="min-w-0 flex-1">
									<p class="truncate text-sm font-semibold text-snow">{displayName(profile)}</p>
									<p class="truncate text-xs text-steel">{username(profile)}</p>
								</div>
								<button class="rounded-md bg-signal px-3 py-2 text-xs font-semibold text-abyss hover:bg-mint disabled:cursor-not-allowed disabled:opacity-60" type="button" disabled={inviteBusy} onclick={() => addMember(friend)}>Add</button>
							</article>
						{/each}
					</div>
				{/if}

				<button class="mt-4 w-full rounded-md border border-warm-charcoal px-4 py-3 text-sm font-semibold text-snow hover:border-signal hover:text-mint disabled:cursor-not-allowed disabled:opacity-60" type="button" disabled={inviteBusy} onclick={createInviteLink}>Create invite link</button>
			</div>
		</div>
	</div>
{/if}

{#snippet MemberRow(member: WorkspaceMemberSummary)}
	{@const profile = memberProfile(member)}
	<article class="flex items-center gap-3 rounded-md px-2 py-2 hover:bg-warm-charcoal/35">
		<span class="relative">
			{@render Avatar(profile)}
			<i class={['absolute right-0 bottom-0 h-3 w-3 rounded-full border-2 border-carbon', profile.userId && presenceById[profile.userId] ? 'bg-signal' : 'bg-steel']}></i>
		</span>
		<div class="min-w-0 flex-1">
			<p class="truncate text-sm font-semibold text-snow">{displayName(profile)}</p>
			<p class="truncate text-xs text-steel">{username(profile)}</p>
		</div>
		{#if profile.userId && profile.userId !== data.sidebar?.viewer?.userId}
			<button class="rounded-md border border-warm-charcoal px-2.5 py-1.5 text-xs font-semibold text-parchment hover:border-red-300/60 hover:text-red-200 disabled:cursor-not-allowed disabled:opacity-60" type="button" disabled={removingMemberId === profile.userId} onclick={() => removeMember(member)}>
				{removingMemberId === profile.userId ? 'Removing...' : 'Remove'}
			</button>
		{/if}
	</article>
{/snippet}

{#snippet Avatar(profile: { userId?: string; username?: string; displayName?: string; avatarUrl?: string })}
	<span class="grid h-10 w-10 shrink-0 place-items-center overflow-hidden rounded-full border border-warm-charcoal bg-[radial-gradient(circle_at_35%_20%,var(--color-mint),var(--color-carbon)_70%)] text-xs font-bold text-snow">
		{#if profile.avatarUrl}
			<img class="h-full w-full object-cover" src={profile.avatarUrl} alt="" />
		{:else}
			{displayName(profile).slice(0, 1).toUpperCase()}
		{/if}
	</span>
{/snippet}
