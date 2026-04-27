import { error, json } from '@sveltejs/kit';

import {
	getChatClient,
	getWorkspaceClient,
	grpcErrorToHttp,
	metadataFromRequest
} from '$lib/grpc/client.server';
import { encodeRouteId } from '$lib/server/route-ids';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request, cookies }) => {
	try {
		const body = await request.json();
		const name = String(body.name ?? '').trim();
		const firstChannelName = String(body.firstChannelName ?? 'general').trim() || 'general';

		if (!name) {
			error(400, 'name is required');
		}

		const metadata = metadataFromRequest(request.headers, cookies);
		const workspace = await getWorkspaceClient().createWorkspace(
			{ name, firstChannelName },
			{ metadata }
		);
		await createChannelConversation(workspace.firstChannelId, metadata);

		return json(
			{
				workspace,
				workspaceRouteId: encodeRouteId(workspace.workspaceId),
				firstChannelRouteId: encodeRouteId(workspace.firstChannelId)
			},
			{ status: 201 }
		);
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
			await getChatClient().createConversation(
				{ targetType: 2, workspaceChannelId: channelId },
				{ metadata }
			);
			return;
		} catch (cause) {
			if (grpcErrorToHttp(cause).status === 409) {
				return;
			}

			lastCause = cause;
			await new Promise((resolve) => setTimeout(resolve, 150));
		}
	}

	throw lastCause;
}
