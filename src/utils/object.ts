export function areFieldsEqual<T extends object, K extends keyof T>(
  left: T,
  right: T,
  keys: readonly K[],
) {
  return keys.every((key) => Object.is(left[key], right[key]));
}
