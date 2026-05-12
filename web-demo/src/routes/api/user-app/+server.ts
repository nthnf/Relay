import { error, json } from '@sveltejs/kit';

import { getBootstrapClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';

import type { RequestHandler } from './$types';

export const GET: RequestHandler = async ({ request, cookies }) => {
	const client = getBootstrapClient();

	try {
		const response = await client.getAppBootstrap(
			{},
			{ metadata: metadataFromRequest(request.headers, cookies) }
		);

		return json(response);
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
