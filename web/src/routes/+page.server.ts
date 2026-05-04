import { accessTokenCookieName } from '$lib/server/auth-cookies';

import type { PageServerLoad } from './$types';

export const load: PageServerLoad = ({ cookies }) => ({
	authenticated: Boolean(cookies.get(accessTokenCookieName()))
});
