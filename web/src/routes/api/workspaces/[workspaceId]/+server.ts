import { error, json } from '@sveltejs/kit';

import { getBootstrapClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';
import { decodeRouteId } from '$lib/server/route-ids';

import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ params, request, cookies }) => {
	try {
		const workspaceId = decodeRouteId(params.workspaceId);
		const response = await getBootstrapClient().getWorkspaceBootstrap(
			{ workspaceId },
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
