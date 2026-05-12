import type { TokenPairResponse } from '../../generated/identity';

const viewer = {
	userId: '11111111-1111-4111-8111-111111111111',
	username: 'demo',
	displayName: 'Demo User',
	avatarUrl: 'https://api.dicebear.com/9.x/shapes/svg?seed=demo'
};

const users = [
	viewer,
	{
		userId: '22222222-2222-4222-8222-222222222222',
		username: 'maya',
		displayName: 'Maya Chen',
		avatarUrl: 'https://api.dicebear.com/9.x/shapes/svg?seed=maya'
	},
	{
		userId: '33333333-3333-4333-8333-333333333333',
		username: 'omar',
		displayName: 'Omar Rivera',
		avatarUrl: 'https://api.dicebear.com/9.x/shapes/svg?seed=omar'
	},
	{
		userId: '44444444-4444-4444-8444-444444444444',
		username: 'nina',
		displayName: 'Nina Patel',
		avatarUrl: 'https://api.dicebear.com/9.x/shapes/svg?seed=nina'
	}
];

const workspace = {
	workspaceId: 'aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa',
	name: 'Relay Demo HQ',
	iconUrl: undefined,
	memberCount: users.length,
	unreadCount: 3
};

const channels = [
	{
		channelId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
		conversationId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
		name: 'general',
		channelKind: 1,
		position: 1,
		unreadCount: 2
	},
	{
		channelId: 'dddddddd-dddd-4ddd-8ddd-dddddddddddd',
		conversationId: 'eeeeeeee-eeee-4eee-8eee-eeeeeeeeeeee',
		name: 'launch-plan',
		channelKind: 1,
		position: 2,
		unreadCount: 1
	}
];

const dmThreads = [
	{
		dmPairId: '55555555-5555-4555-8555-555555555555',
		conversationId: '66666666-6666-4666-8666-666666666666',
		peerUserId: users[1].userId,
		peerUsername: users[1].username,
		peerDisplayName: users[1].displayName,
		peerAvatarUrl: users[1].avatarUrl,
		unreadCount: 1
	},
	{
		dmPairId: '77777777-7777-4777-8777-777777777777',
		conversationId: '88888888-8888-4888-8888-888888888888',
		peerUserId: users[2].userId,
		peerUsername: users[2].username,
		peerDisplayName: users[2].displayName,
		peerAvatarUrl: users[2].avatarUrl,
		unreadCount: 0
	}
];

type DemoMessage = ReturnType<typeof message>;

let messageSeq = 10;
const messages: DemoMessage[] = [
	message('m-1', channels[0].conversationId, users[1].userId, 'Welcome to the backend-free Relay demo.', 1, -42),
	message('m-2', channels[0].conversationId, viewer.userId, 'Everything here is rendered from local dummy data.', 2, -34),
	message('m-3', channels[0].conversationId, users[2].userId, 'You can send, edit, and delete messages during this session.', 3, -18),
	message('m-4', channels[1].conversationId, users[3].userId, 'Launch checklist is green for the public demo.', 1, -55),
	message('m-5', dmThreads[0].conversationId, users[1].userId, 'Want to review the invite flow?', 1, -15),
	message('m-6', dmThreads[1].conversationId, users[2].userId, 'The Kubernetes backend is not required here.', 1, -8)
];

function message(id: string, conversationId: string, authorUserId: string, body: string, seq: number, minutesAgo: number) {
	return {
		messageId: id,
		conversationId,
		authorUserId,
		body,
		conversationMessageSeq: seq,
		createdAt: new Date(Date.now() + minutesAgo * 60_000),
		updatedAt: undefined as Date | undefined,
		deletedAt: undefined as Date | undefined
	};
}

export function demoTokenPair(): TokenPairResponse {
	const now = Date.now();
	return {
		userId: viewer.userId,
		sessionId: 'demo-session',
		accessToken: 'demo-access-token',
		accessTokenExpiresAt: new Date(now + 24 * 60 * 60_000),
		refreshToken: 'demo-refresh-token',
		refreshTokenExpiresAt: new Date(now + 30 * 24 * 60 * 60_000),
		emailVerified: true,
		profile: viewer
	};
}

export const demoIdentityClient = {
	authenticatePassword: async () => demoTokenPair(),
	refreshSession: async () => demoTokenPair(),
	redeemEmailVerificationToken: async () => demoTokenPair(),
	revokeSession: async () => ({}),
	registerUser: async () => ({ ok: true }),
	resendVerificationEmail: async () => ({ ok: true }),
	updateUserProfile: async () => ({}),
	getUsersByIds: async ({ userIds }: { userIds: string[] }) => ({
		users: users.filter((user) => userIds.includes(user.userId))
	})
};

export const demoBootstrapClient = {
	getAppBootstrap: async () => ({
		viewer,
		workspaces: [workspace],
		pendingFriendRequestCount: 1
	}),
	getWorkspaceBootstrap: async ({ workspaceId }: { workspaceId: string }) => ({
		workspace: workspace.workspaceId === workspaceId ? workspace : undefined,
		channels: workspace.workspaceId === workspaceId ? channels : []
	}),
	getDmBootstrap: async () => ({ items: dmThreads })
};

export const demoChatClient = {
	createConversation: async ({ targetType, peerUserId, workspaceChannelId }: { targetType?: number; peerUserId?: string; workspaceChannelId?: string }) => {
		if (targetType === 1 && peerUserId) {
			return { conversationId: dmThreads.find((thread) => thread.peerUserId === peerUserId)?.conversationId ?? dmThreads[0].conversationId };
		}

		return { conversationId: channels.find((channel) => channel.channelId === workspaceChannelId)?.conversationId ?? channels[0].conversationId };
	},
	listConversationMessages: async ({ conversationId }: { conversationId: string }) => ({
		messages: messages.filter((item) => item.conversationId === conversationId && !item.deletedAt)
	}),
	createMessage: async ({ conversationId, body }: { conversationId: string; body: string }) => {
		const created = message(`demo-${Date.now()}`, conversationId, viewer.userId, body, ++messageSeq, 0);
		messages.push(created);
		return created;
	},
	editMessage: async ({ messageId, newBody }: { messageId: string; newBody: string }) => {
		const existing = messages.find((item) => item.messageId === messageId);
		if (existing) {
			existing.body = newBody;
			existing.updatedAt = new Date();
		}
		return existing ?? {};
	},
	deleteMessage: async ({ messageId }: { messageId: string }) => {
		const existing = messages.find((item) => item.messageId === messageId);
		if (existing) {
			existing.deletedAt = new Date();
		}
		return existing ?? {};
	},
	markConversationRead: async () => ({})
};

export const demoWorkspaceClient = {
	createWorkspace: async ({ name, firstChannelName }: { name: string; firstChannelName?: string }) => ({
		workspaceId: workspace.workspaceId,
		name,
		firstChannelId: channels[0].channelId,
		firstChannelName: firstChannelName ?? channels[0].name
	}),
	getWorkspace: async () => workspace,
	updateWorkspace: async ({ name }: { name?: string }) => ({ ...workspace, name: name ?? workspace.name }),
	deleteWorkspace: async () => ({}),
	joinWorkspaceByInviteLink: async () => ({ workspaceId: workspace.workspaceId }),
	listChannels: async () => ({ channels }),
	createChannel: async ({ name }: { name: string }) => ({
		channelId: channels[0].channelId,
		name,
		channelKind: 1,
		position: channels.length + 1
	}),
	listWorkspaceMembers: async () => ({
		members: users.map((user) => ({ userId: user.userId, profile: user, role: user.userId === viewer.userId ? 'owner' : 'member' }))
	}),
	addMember: async () => ({}),
	removeMember: async () => ({}),
	createInviteLink: async () => ({ code: 'demo-invite' })
};

export const demoFriendshipClient = {
	listFriends: async () => ({
		friends: users.slice(1, 3).map((user) => ({ friendUserId: user.userId }))
	}),
	listPendingRequests: async ({ direction }: { direction: string }) => ({
		requests:
			direction === 'incoming'
				? [{ friendRequestId: 'request-1', requesterUserId: users[3].userId, requester: users[3] }]
				: []
	}),
	listBlockedUsers: async () => ({ blockedUsers: [] }),
	createFriendRequest: async () => ({}),
	acceptFriendRequest: async () => ({}),
	rejectFriendRequest: async () => ({}),
	removeFriend: async () => ({}),
	blockUser: async () => ({}),
	unblockUser: async () => ({})
};

export const demoRealtimeClient = {
	getUserPresence: async ({ userIds }: { userIds: string[] }) => ({
		users: userIds.map((userId, index) => ({
			userId,
			online: index % 2 === 0,
			lastSeenAt: new Date(Date.now() - index * 12 * 60_000)
		}))
	})
};
