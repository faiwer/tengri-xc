import { z } from 'zod';

/** `launch_method` enum — how the flight got airborne. */
export const LaunchMethodIo = z.enum(['foot', 'winch', 'aerotow']);
export type LaunchMethod = z.infer<typeof LaunchMethodIo>;

/** `propulsion` enum — how the flight stayed airborne. */
export const PropulsionIo = z.enum(['free', 'self_launch', 'powered']);
export type Propulsion = z.infer<typeof PropulsionIo>;
