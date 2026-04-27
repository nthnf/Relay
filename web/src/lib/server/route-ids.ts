const uuidPattern = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{12}$/i;

export function encodeRouteId(uuid: string): string {
	assertUuid(uuid);
	return Buffer.from(uuid.replaceAll('-', ''), 'hex').toString('base64url');
}

export function decodeRouteId(routeId: string): string {
	const hex = Buffer.from(routeId, 'base64url').toString('hex');

	if (hex.length !== 32) {
		throw new Error('invalid route id');
	}

	const uuid = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
	assertUuid(uuid);
	return uuid;
}

function assertUuid(value: string): void {
	if (!uuidPattern.test(value)) {
		throw new Error('invalid uuid');
	}
}
