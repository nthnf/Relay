import { fail, redirect } from '@sveltejs/kit';

import { getBootstrapClient, getChatClient, getWorkspaceClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { encodeRouteId } from '$lib/server/route-ids';

import type { Actions } from './$types';

export const actions: Actions = {
	default: async ({ request, cookies }) => {
		const formData = await request.formData();
		const name = String(formData.get('name') ?? '').trim();
		const firstChannelName = String(formData.get('firstChannelName') ?? 'general').trim() || 'general';

		if (!name) {
			return fail(400, { error: 'Server name is required', name, firstChannelName });
		}

		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			const workspace = await getWorkspaceClient().createWorkspace({ name, firstChannelName }, { metadata });
			const conversationId = await createChannelConversation(workspace.firstChannelId, metadata);
			await waitForProjectedChannel(workspace.workspaceId, workspace.firstChannelId, metadata);
			const redirectTo = `/workspace/${encodeRouteId(workspace.workspaceId)}/${encodeRouteId(workspace.firstChannelId)}?conversationId=${encodeURIComponent(conversationId)}`;
			redirect(303, redirectTo);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message, name, firstChannelName });
		}
	}
};

async function createChannelConversation(channelId: string, metadata: ReturnType<typeof metadataFromRequest>) {
	let lastCause: unknown;
	const delays = [1000, 2000, 4000, 8000];

	for (let attempt = 0; attempt <= delays.length; attempt += 1) {
		try {
			const conversation = await getChatClient().createConversation({ targetType: 2, workspaceChannelId: channelId }, { metadata });
			return conversation.conversationId;
		} catch (cause) {
			lastCause = cause;

			if (attempt < delays.length) {
				await new Promise((resolve) => setTimeout(resolve, delays[attempt]));
			}
		}
	}

	throw lastCause;
}

async function waitForProjectedChannel(
	workspaceId: string,
	channelId: string,
	metadata: ReturnType<typeof metadataFromRequest>
) {
	const delays = [1000, 2000, 4000, 8000];

	for (let attempt = 0; attempt <= delays.length; attempt += 1) {
		const bootstrap = await getBootstrapClient().getWorkspaceBootstrap({ workspaceId }, { metadata });
		const channel = bootstrap.channels.find((item) => item.channelId === channelId);

		if (channel?.conversationId) {
			return;
		}

		if (attempt < delays.length) {
			await new Promise((resolve) => setTimeout(resolve, delays[attempt]));
		}
	}
}
