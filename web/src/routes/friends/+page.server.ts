import { error, fail, redirect } from '@sveltejs/kit';

import {
	getBootstrapClient,
	getChatClient,
	getFriendshipClient,
	getIdentityClient,
	grpcErrorToHttp,
	metadataFromRequest
} from '$lib/grpc/client.server';
import { decodeRouteId, encodeRouteId } from '$lib/server/route-ids';

import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ request, cookies }) => {
	const metadata = metadataFromRequest(request.headers, cookies);

	try {
		const [friends, incoming, outgoing] = await Promise.all([
			getFriendshipClient().listFriends({ pageSize: 100 }, { metadata }),
			getFriendshipClient().listPendingRequests(
				{ direction: 'incoming', pageSize: 100 },
				{ metadata }
			),
			getFriendshipClient().listPendingRequests(
				{ direction: 'outgoing', pageSize: 100 },
				{ metadata }
			)
		]);
		const users = friends.friends.length
			? await getIdentityClient().getUsersByIds(
					{ userIds: friends.friends.map((friend) => friend.friendUserId) },
					{ metadata }
				)
			: { users: [] };
		const profilesById = new Map(users.users.map((user) => [user.userId, user]));

		return {
			friends: friends.friends.map((friend) => ({
				...friend,
				routeUserId: encodeRouteId(friend.friendUserId),
				profile: profilesById.get(friend.friendUserId)
			})),
			incoming: incoming.requests,
			outgoing: outgoing.requests
		};
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

export const actions: Actions = {
	createDm: async ({ request, cookies }) => {
		const formData = await request.formData();
		const peerUserId = decodeFormRouteId(formData.get('peerUserId'));

		if (!peerUserId) {
			return fail(400, { error: 'Friend is required' });
		}

		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			await getChatClient().createConversation(
				{ targetType: 1, peerUserId },
				{ metadata }
			);
			const thread = await waitForDmThread(peerUserId, metadata);

			if (!thread) {
				return fail(503, { error: 'DM created. Refresh after projections catch up.' });
			}

			redirect(303, `/dm/${encodeRouteId(thread.dmPairId)}`);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message });
		}
	},
	requestFriend: async ({ request, cookies }) => {
		const formData = await request.formData();
		const targetUserId = String(formData.get('targetUserId') ?? '').trim();

		if (!targetUserId) {
			return fail(400, { error: 'Target user ID is required' });
		}

		try {
			await getFriendshipClient().createFriendRequest(
				{ targetUserId },
				{ metadata: metadataFromRequest(request.headers, cookies) }
			);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message });
		}

		redirect(303, '/friends');
	},
	acceptRequest: async ({ request, cookies }) => {
		const friendRequestId = stringFormValue((await request.formData()).get('friendRequestId'));

		if (!friendRequestId) {
			return fail(400, { error: 'Friend request is required' });
		}

		try {
			await getFriendshipClient().acceptFriendRequest(
				{ friendRequestId },
				{ metadata: metadataFromRequest(request.headers, cookies) }
			);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message });
		}

		redirect(303, '/friends');
	},
	rejectRequest: async ({ request, cookies }) => {
		const friendRequestId = stringFormValue((await request.formData()).get('friendRequestId'));

		if (!friendRequestId) {
			return fail(400, { error: 'Friend request is required' });
		}

		try {
			await getFriendshipClient().rejectFriendRequest(
				{ friendRequestId },
				{ metadata: metadataFromRequest(request.headers, cookies) }
			);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message });
		}

		redirect(303, '/friends');
	},
	removeFriend: async ({ request, cookies }) => {
		const friendUserId = decodeFormRouteId((await request.formData()).get('friendUserId'));

		if (!friendUserId) {
			return fail(400, { error: 'Friend is required' });
		}

		try {
			await getFriendshipClient().removeFriend(
				{ friendUserId },
				{ metadata: metadataFromRequest(request.headers, cookies) }
			);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message });
		}

		redirect(303, '/friends');
	},
	blockUser: async ({ request, cookies }) => {
		const targetUserId = decodeFormRouteId((await request.formData()).get('targetUserId'));

		if (!targetUserId) {
			return fail(400, { error: 'User is required' });
		}

		try {
			await getFriendshipClient().blockUser(
				{ targetUserId },
				{ metadata: metadataFromRequest(request.headers, cookies) }
			);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message });
		}

		redirect(303, '/friends');
	},
	unblockUser: async ({ request, cookies }) => {
		const targetUserId = stringFormValue((await request.formData()).get('targetUserId'));

		if (!targetUserId) {
			return fail(400, { error: 'Target user ID is required' });
		}

		try {
			await getFriendshipClient().unblockUser(
				{ targetUserId },
				{ metadata: metadataFromRequest(request.headers, cookies) }
			);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message });
		}

		redirect(303, '/friends');
	}
};

async function waitForDmThread(
	peerUserId: string,
	metadata: ReturnType<typeof metadataFromRequest>
) {
	for (let attempt = 0; attempt < 6; attempt += 1) {
		const bootstrap = await getBootstrapClient().getDmBootstrap({}, { metadata });
		const thread = bootstrap.items.find((item) => item.peerUserId === peerUserId);

		if (thread) {
			return thread;
		}

		await new Promise((resolve) => setTimeout(resolve, 150));
	}
}

function decodeFormRouteId(value: FormDataEntryValue | null): string | undefined {
	if (typeof value !== 'string') {
		return undefined;
	}

	try {
		return decodeRouteId(value);
	} catch {
		return undefined;
	}
}

function stringFormValue(value: FormDataEntryValue | null): string | undefined {
	if (typeof value !== 'string') {
		return undefined;
	}

	return value.trim() || undefined;
}
