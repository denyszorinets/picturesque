/** Whether `value` is a whole percentage from 0 to 100 (`null` when the field is empty). */
export function validPercent(value: number | null | undefined): boolean {
  return (
    typeof value === "number" &&
    Number.isInteger(value) &&
    value >= 0 &&
    value <= 100
  );
}
