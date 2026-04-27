import { error, json } from '@sveltejs/kit';

import {
	getBootstrapClient,
	getChatClient,
	getWorkspaceClient,
	grpcErrorToHttp,
	metadataFromRequest
} from '$lib/grpc/client.server';
import { decodeRouteId } from '$lib/server/route-ids';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ params, request, cookies }) => {
	try {
		const workspaceId = decodeRouteId(params.workspaceId);
		const body = await request.json();
		const name = String(body.name ?? '').trim();
		const channelKind = String(body.channelKind ?? 'text').trim() || 'text';

		if (!name) {
			error(400, 'name is required');
		}

		const metadata = metadataFromRequest(request.headers, cookies);
		const channel = await getWorkspaceClient().createChannel(
			{ workspaceId, name, channelKind },
			{ metadata }
		);
		const conversation = await createChannelConversation(channel.channelId, metadata);
		const projected = await waitForProjectedChannel(workspaceId, channel.channelId, metadata);

		return json({ channel, conversation, projected }, { status: 201 });
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

async function createChannelConversation(
	channelId: string,
	metadata: ReturnType<typeof metadataFromRequest>
) {
	let lastCause: unknown;

	for (let attempt = 0; attempt < 6; attempt += 1) {
		try {
			return await getChatClient().createConversation(
				{ targetType: 2, workspaceChannelId: channelId },
				{ metadata }
			);
		} catch (cause) {
			if (grpcErrorToHttp(cause).status === 409) {
				return undefined;
			}

			lastCause = cause;
			await new Promise((resolve) => setTimeout(resolve, 150));
		}
	}

	throw lastCause;
}

async function waitForProjectedChannel(
	workspaceId: string,
	channelId: string,
	metadata: ReturnType<typeof metadataFromRequest>
) {
	for (let attempt = 0; attempt < 6; attempt += 1) {
		const bootstrap = await getBootstrapClient().getWorkspaceBootstrap({ workspaceId }, { metadata });
		const channel = bootstrap.channels.find((item) => item.channelId === channelId);

		if (channel?.conversationId) {
			return channel;
		}

		await new Promise((resolve) => setTimeout(resolve, 150));
	}
}
