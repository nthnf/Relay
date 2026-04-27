import { error, fail, redirect } from '@sveltejs/kit';

import {
	getBootstrapClient,
	getChatClient,
	getWorkspaceClient,
	grpcErrorToHttp,
	metadataFromRequest
} from '$lib/grpc/client.server';
import { decodeRouteId, encodeRouteId } from '$lib/server/route-ids';

import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, request, cookies }) => {
	const workspaceId = decodeParam(params.workspaceId);
	const metadata = metadataFromRequest(request.headers, cookies);

	try {
		const bootstrap = await getBootstrapClient().getWorkspaceBootstrap({ workspaceId }, { metadata });

		if (!bootstrap.workspace) {
			error(404, 'Workspace not found');
		}

		return {
			workspace: bootstrap.workspace,
			workspaceRouteId: params.workspaceId,
			channels: bootstrap.channels.map((channel) => ({
				...channel,
				routeId: encodeRouteId(channel.channelId)
			}))
		};
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

export const actions: Actions = {
	createChannel: async ({ params, request, cookies }) => {
		const formData = await request.formData();
		const name = String(formData.get('name') ?? '').trim();
		const channelKind = String(formData.get('channelKind') ?? 'text').trim() || 'text';

		if (!name) {
			return fail(400, { name, error: 'Channel name is required' });
		}

		const workspaceId = decodeParam(params.workspaceId);
		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			const channel = await getWorkspaceClient().createChannel(
				{ workspaceId, name, channelKind },
				{ metadata }
			);
			await createChannelConversation(channel.channelId, metadata);
			const projected = await waitForProjectedChannel(workspaceId, channel.channelId, metadata);

			if (!projected) {
				return fail(503, { error: 'Channel created. Refresh after projections catch up.' });
			}

			redirect(303, `/workspace/${params.workspaceId}/${encodeRouteId(channel.channelId)}`);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { name, error: message });
		}
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

function decodeParam(value: string): string {
	try {
		return decodeRouteId(value);
	} catch {
		error(400, 'Invalid route id');
	}
}
