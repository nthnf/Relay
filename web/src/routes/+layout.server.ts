import { getBootstrapClient, metadataFromRequest } from '$lib/grpc/client.server';
import { encodeRouteId } from '$lib/server/route-ids';

import type { LayoutServerLoad } from './$types';

export const load: LayoutServerLoad = async ({ request, cookies, url }) => {
	if (url.pathname.startsWith('/auth/')) {
		return { sidebar: null };
	}

	try {
		const metadata = metadataFromRequest(request.headers, cookies);
		const [app, dms] = await Promise.all([
			getBootstrapClient().getAppBootstrap({}, { metadata }),
			getBootstrapClient().getDmBootstrap({}, { metadata })
		]);

		const workspaceBootstraps = await Promise.all(
			app.workspaces.map((workspace) =>
				getBootstrapClient().getWorkspaceBootstrap({ workspaceId: workspace.workspaceId }, { metadata })
			)
		);

		return {
			sidebar: {
				viewer: app.viewer,
				workspaces: app.workspaces.map((workspace, index) => ({
					...workspace,
					routeId: encodeRouteId(workspace.workspaceId),
					firstChannelRouteId: workspaceBootstraps[index]?.channels[0]
						? encodeRouteId(workspaceBootstraps[index].channels[0].channelId)
						: undefined
				})),
				dms: dms.items.map((dm) => ({ ...dm, routeId: encodeRouteId(dm.dmPairId) })),
				pendingFriendRequestCount: app.pendingFriendRequestCount
			}
		};
	} catch {
		return { sidebar: null };
	}
};
