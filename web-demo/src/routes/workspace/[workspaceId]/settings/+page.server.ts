import { error, fail, redirect } from '@sveltejs/kit';

import { getBootstrapClient, getWorkspaceClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { decodeRouteId, encodeRouteId } from '$lib/server/route-ids';

import type { Actions, PageServerLoad } from './$types';

export const load: PageServerLoad = async ({ params, request, cookies }) => {
	const workspaceId = decodeParam(params.workspaceId);
	const metadata = metadataFromRequest(request.headers, cookies);

	try {
		const [appBootstrap, workspaceBootstrap, workspaceDetails] = await Promise.all([
			getBootstrapClient().getAppBootstrap({}, { metadata }),
			getBootstrapClient().getWorkspaceBootstrap({ workspaceId }, { metadata }),
			getWorkspaceClient().getWorkspace({ workspaceId }, { metadata })
		]);

		if (!workspaceBootstrap.workspace) {
			error(404, 'Workspace not found');
		}

		return {
			workspace: {
				...workspaceBootstrap.workspace,
				name: workspaceDetails.name
			},
			viewerUserId: appBootstrap.viewer?.userId,
			isOwner: appBootstrap.viewer?.userId === workspaceDetails.ownerUserId,
			workspaceRouteId: params.workspaceId,
			channels: workspaceBootstrap.channels.map((item) => ({ ...item, routeId: encodeRouteId(item.channelId) }))
		};
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};

export const actions: Actions = {
	update: async ({ params, request, cookies }) => {
		const workspaceId = decodeParam(params.workspaceId);
		const formData = await request.formData();
		const name = String(formData.get('name') ?? '').trim();
		const iconUrl = String(formData.get('iconUrl') ?? '').trim();

		if (!name) {
			return fail(400, { updateError: 'Workspace name is required', name, iconUrl });
		}

		try {
			await getWorkspaceClient().updateWorkspace(
				{ workspaceId, name, iconUrl: iconUrl || undefined },
				{ metadata: metadataFromRequest(request.headers, cookies) }
			);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { updateError: message, name, iconUrl });
		}

		return { updated: true, name, iconUrl };
	},

	delete: async ({ params, request, cookies }) => {
		const workspaceId = decodeParam(params.workspaceId);
		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			await getWorkspaceClient().deleteWorkspace({ workspaceId }, { metadata });
			await waitForWorkspaceRemoval(workspaceId, metadata);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { deleteError: message });
		}

		redirect(303, '/dm');
	},

	leave: async ({ params, request, cookies }) => {
		const workspaceId = decodeParam(params.workspaceId);
		const metadata = metadataFromRequest(request.headers, cookies);

		try {
			const app = await getBootstrapClient().getAppBootstrap({}, { metadata });
			const viewerUserId = app.viewer?.userId;

			if (!viewerUserId) {
				return fail(401, { leaveError: 'Authentication required' });
			}

			await getWorkspaceClient().removeMember({ workspaceId, targetUserId: viewerUserId }, { metadata });
			await waitForWorkspaceRemoval(workspaceId, metadata);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { leaveError: message });
		}

		redirect(303, '/dm');
	}
};

async function waitForWorkspaceRemoval(
	workspaceId: string,
	metadata: ReturnType<typeof metadataFromRequest>
): Promise<void> {
	for (let attempt = 0; attempt < 20; attempt += 1) {
		const app = await getBootstrapClient().getAppBootstrap({}, { metadata });

		if (!app.workspaces.some((workspace) => workspace.workspaceId === workspaceId)) {
			return;
		}

		await new Promise((resolve) => setTimeout(resolve, 100));
	}
}

function decodeParam(value: string): string {
	try {
		return decodeRouteId(value);
	} catch {
		error(400, 'Invalid route id');
	}
}
