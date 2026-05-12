<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { env } from '$env/dynamic/public';
	import ChatHeader from '$lib/components/chat/ChatHeader.svelte';
	import ChatMessageList from '$lib/components/chat/ChatMessageList.svelte';
	import DmSidebar from '$lib/components/chat/DmSidebar.svelte';
	import MessageComposer from '$lib/components/chat/MessageComposer.svelte';
	import PrimarySidebar from '$lib/components/chat/PrimarySidebar.svelte';
	import { onMount } from 'svelte';
	import type { ActionData, PageData } from './$types';

	let { data, form }: { data: PageData; form: ActionData } = $props();

	let authorNames = $derived(
		new Map(data.authorProfiles.map((profile) => [profile.userId, profile.displayName || profile.username || profile.userId]))
	);
	let messages = $derived(
		data.messages.messages.map((message) => ({
			...message,
			senderName: authorNames.get(message.authorUserId) ?? 'Unknown user',
			outgoing: Boolean(data.sidebar?.viewer?.userId) && message.authorUserId === data.sidebar?.viewer?.userId
		}))
	);
	let lastReadSeq = $derived(data.messages.messages.at(-1)?.conversationMessageSeq);

	async function markConversationRead() {
		if (!data.thread.conversationId || lastReadSeq === undefined) {
			return;
		}

		await fetch(`/api/conversations/${data.thread.conversationId}/read`, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify({ lastReadConversationMessageSeq: Number(lastReadSeq) })
		}).catch(() => undefined);

		setTimeout(() => void invalidateAll(), 250);
	}

	onMount(() => {
		markConversationRead();

		const viewerUserId = data.sidebar?.viewer?.userId;
		const targetId = data.thread.conversationId;

		if (!viewerUserId || !targetId) {
			return;
		}

		const socket = new WebSocket(`${env.PUBLIC_REALTIME_WS_URL || 'ws://localhost:30080/ws'}?user_id=${encodeURIComponent(viewerUserId)}`);
		socket.addEventListener('open', () => {
			socket.send(JSON.stringify({ type: 'subscribe', target_kind: 'direct_message', target_id: targetId }));
		});
		socket.addEventListener('message', () => void invalidateAll());
		socket.addEventListener('error', () => undefined);

		return () => socket.close();
	});
</script>

<svelte:head><title>DM with {data.thread.peerDisplayName}</title></svelte:head>

<div class="flex min-h-screen bg-abyss text-snow">
	<PrimarySidebar sidebar={data.sidebar} active="dm" />
	<DmSidebar threads={data.sidebar?.dms ?? []} activeDmPairId={data.thread.dmPairId} />

	<main class="grid min-h-screen min-w-0 flex-1 grid-rows-[auto_1fr_auto] bg-chat-main">
		<ChatHeader
			title={data.thread.peerDisplayName || data.thread.peerUsername || 'Direct message'}
			subtitle={`@${data.thread.peerUsername}`}
			avatarText={data.thread.peerDisplayName || data.thread.peerUsername || '?'}
			variant="dm"
		/>
		<ChatMessageList {messages} variant="dm" showDatePill currentUserId={data.sidebar?.viewer?.userId} />
		<MessageComposer error={form?.error} body={form?.body ?? ''} />
	</main>
</div>
