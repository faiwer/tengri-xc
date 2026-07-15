import { z } from 'zod';

/** `launch_method` enum — how the flight got airborne. */
export const LaunchMethodIo = z.enum(['foot', 'winch', 'aerotow']);
export type LaunchMethod = z.infer<typeof LaunchMethodIo>;
