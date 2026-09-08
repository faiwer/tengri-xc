import type { Rule } from 'antd/es/form';

/**
 * Format rules for a login, mirroring the server's `validate_login`. `required`
 * is left out so each form can word it in its own voice.
 *
 * `@` is deliberately excluded: sign-in matches one identifier against `login`
 * *or* `email`, so an address-shaped login makes the row it resolves to
 * ambiguous.
 */
export const LOGIN_RULES: Rule[] = [
  {
    pattern: /^[A-Za-z0-9._-]{3,32}$/,
    message: '3–32 letters, digits, periods, hyphens, or underscores',
  },
];

/**
 * Strength rules for a new password, mirroring the server's `weak_password`.
 * `required` is left out for the same reason as {@link LOGIN_RULES}.
 */
export const PASSWORD_RULES: Rule[] = [
  { min: 8, message: 'At least 8 characters' },
  {
    // At least one letter and one digit anywhere; `min` above covers length.
    pattern: /^(?=.*[A-Za-z])(?=.*\d).+$/,
    message: 'Must include a letter and a digit',
  },
];
