import type { Sport } from '../../../api/admin/gliders.io';
import type { LaunchMethod, Propulsion } from '../../../api/flights.io';

/** Working form state — fields are nullable until the pilot fills them. */
export interface FlightDetailsForm {
  kind: Sport;
  brandId: string | null;
  modelId: string | null;
  launchMethod: LaunchMethod | null;
  propulsion: Propulsion | null;
}

/** Resolved value emitted on submit — every field is present. */
export interface FlightDetails {
  kind: Sport;
  brandId: string;
  modelId: string;
  launchMethod: LaunchMethod;
  propulsion: Propulsion;
}
