import { error, json } from '@sveltejs/kit';

import {
	getFriendshipClient,
	getIdentityClient,
	grpcErrorToHttp,
	metadataFromRequest
} from '$lib/grpc/client.server';

import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ request, cookies, url }) => {
	const pageSize = optionalInteger(url.searchParams.get('pageSize')) ?? 100;
	const pageToken = url.searchParams.get('pageToken') ?? undefined;
	const metadata = metadataFromRequest(request.headers, cookies);

	try {
		const [friends, blocked] = await Promise.all([
			getFriendshipClient().listFriends({ pageSize, pageToken }, { metadata }),
			getFriendshipClient().listBlockedUsers({ pageSize, pageToken: undefined }, { metadata })
		]);
		const profiles = friends.friends.length
			? await getIdentityClient().getUsersByIds(
					{ userIds: friends.friends.map((friend) => friend.friendUserId) },
					{ metadata }
				)
			: { users: [] };

		return json({ ...friends, profiles: profiles.users, blockedUsers: blocked.blockedUsers });
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

function optionalInteger(value: string | null): number | undefined {
	if (!value) {
		return undefined;
	}

	const parsed = Number(value);
	return Number.isInteger(parsed) ? parsed : undefined;
}
