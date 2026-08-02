import { Select, type SelectProps } from 'antd';
import { useMemo } from 'react';

import { countryOptions } from '../../utils/formatCountry';

export type CountrySelectProps = Omit<SelectProps<string>, 'options'>;

/**
 * ISO 3166-1 alpha-2 country picker: an AntD `Select` prefilled with every
 * valid country (flag + localized name), searchable by name. `value`/`onChange`
 * pass through so it drops straight into an AntD `Form.Item`.
 */
export function CountrySelect(props: CountrySelectProps) {
  const options = useMemo(
    () =>
      countryOptions().map((country) => ({
        value: country.code,
        label: country.label,
      })),
    [],
  );

  return (
    <Select
      allowClear
      showSearch
      optionFilterProp="label"
      options={options}
      {...props}
    />
  );
}
