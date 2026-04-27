import { error, json } from '@sveltejs/kit';

import { getIdentityClient, grpcErrorToHttp } from '$lib/grpc/client.server';

import type { RequestHandler } from './$types';

export const POST: RequestHandler = async ({ request }) => {
	try {
		const body = await request.json();
		const response = await getIdentityClient().registerUser(body);

		return json(response, { status: 201 });
	} catch (cause) {
		const { status, message } = grpcErrorToHttp(cause);
		error(status, message);
	}
};
