/**
 * A short random id. Keeps logins, addresses, and anything else that has to
 * be unique from colliding across runs against one E2E database.
 */
export const makeId = (): string => crypto.randomUUID().slice(0, 8);
