import { error, fail, redirect } from '@sveltejs/kit';

import {
	getBootstrapClient,
	getChatClient,
	getWorkspaceClient,
	grpcErrorToHttp,
	metadataFromRequest
} from '$lib/grpc/client.server';
import { encodeRouteId } from '$lib/server/route-ids';

import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ request, cookies }) => {
	try {
		const app = await getBootstrapClient().getAppBootstrap(
			{},
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return {
			viewer: app.viewer,
			workspaces: app.workspaces.map((workspace) => ({
				...workspace,
				routeId: encodeRouteId(workspace.workspaceId)
			})),
			pendingFriendRequestCount: app.pendingFriendRequestCount
		};
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

export const actions: Actions = {
	createWorkspace: async ({ request, cookies }) => {
		const formData = await request.formData();
		const name = String(formData.get('name') ?? '').trim();
		const firstChannelName = String(formData.get('firstChannelName') ?? 'general').trim() || 'general';

		if (!name) {
			return fail(400, { error: 'Workspace name is required' });
		}

		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			const workspace = await getWorkspaceClient().createWorkspace(
				{ name, firstChannelName },
				{ metadata }
			);
			await createChannelConversation(workspace.firstChannelId, metadata);
			redirect(303, `/workspace/${encodeRouteId(workspace.workspaceId)}`);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message });
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
