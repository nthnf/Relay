import { fail, redirect } from '@sveltejs/kit';

import { getIdentityClient, grpcErrorToHttp, metadataFromRequest } from '$lib/grpc/client.server';

import type { Actions } from './$types';

export const actions: Actions = {
	update: async ({ request, cookies }) => {
		const formData = await request.formData();
		const displayName = String(formData.get('displayName') ?? '').trim();
		const avatarUrl = String(formData.get('avatarUrl') ?? '').trim();

		if (!displayName) {
			return fail(400, { error: 'Display name is required', displayName, avatarUrl });
		}

		try {
			await getIdentityClient().updateUserProfile(
				{ displayName, avatarUrl: avatarUrl || undefined },
				{ metadata: metadataFromRequest(request.headers, cookies) }
			);
		} catch (cause) {
			const { status, message } = grpcErrorToHttp(cause);
			return fail(status, { error: message, displayName, avatarUrl });
		}

		redirect(303, '/profile');
	}
};
